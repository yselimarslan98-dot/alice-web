use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use zip::ZipArchive;
#[cfg(not(target_arch = "wasm32"))]
use std::fs::File;

#[cfg(not(target_arch = "wasm32"))]
type Pack = ZipArchive<File>;
/// The browser reads the same PK3 bytes through web/worker.js. Entry reads are prefetched
/// whole (see `web::Window`); the buffer only serves central-directory fallbacks.
#[cfg(target_arch = "wasm32")]
type Pack = ZipArchive<std::io::BufReader<web::PackFile>>;

#[cfg(not(target_arch = "wasm32"))]
fn open_pack(path: &Path) -> Result<Pack> {
    Ok(ZipArchive::new(File::open(path)?)?)
}
#[cfg(target_arch = "wasm32")]
fn open_pack(path: &Path) -> Result<(Pack, web::Window)> {
    let file = web::PackFile::open(path)?;
    let window = file.window();
    Ok((
        ZipArchive::new(std::io::BufReader::with_capacity(64 * 1024, file))?,
        window,
    ))
}

const MAX_ENTRY: u64 = 128 * 1024 * 1024;

/// Read-only PK3 mount. Later alphabetically sorted packs override earlier packs.
pub struct Assets {
    packs: Vec<Pack>,
    paths: Vec<PathBuf>,
    entries: BTreeMap<String, (usize, usize)>,
    pub base: PathBuf,
    /// Browser build: (size, crc) per entry from each central directory. `by_index` re-reads
    /// the local header every call, which is cheap natively but a bridge round trip here.
    #[cfg(target_arch = "wasm32")]
    sizes: Vec<Vec<(u64, u32)>>,
    /// Browser build: (local header offset, bytes to the end of the compressed data) per entry.
    #[cfg(target_arch = "wasm32")]
    spans: Vec<Vec<(u64, u64)>>,
    #[cfg(target_arch = "wasm32")]
    windows: Vec<web::Window>,
}

impl Assets {
    /// Stable identity of the mounted, override-resolved corpus, independent of its folder.
    /// Central-directory CRC/size metadata avoids decoding every asset at startup.
    pub fn fingerprint(&mut self) -> Result<String> {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        for (name, &(pack, index)) in &self.entries {
            #[cfg(not(target_arch = "wasm32"))]
            let (size, crc) = {
                let entry = self.packs[pack].by_index(index)?;
                (entry.size(), entry.crc32())
            };
            #[cfg(target_arch = "wasm32")]
            let (size, crc) = self.sizes[pack][index];
            hash.update((name.len() as u64).to_le_bytes());
            hash.update(name.as_bytes());
            hash.update(size.to_le_bytes());
            hash.update(crc.to_le_bytes());
        }
        Ok(format!("{:x}", hash.finalize()))
    }
    #[cfg(target_arch = "wasm32")]
    pub fn open(base: &Path) -> Result<Self> {
        Self::open_paths(base, web::pack_paths(base)?)
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(base: &Path) -> Result<Self> {
        let paths = fs::read_dir(base)
            .with_context(|| {
                format!(
                    "Cannot read game data at {}. Pass --data <base folder>.",
                    base.display()
                )
            })?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|s| s.eq_ignore_ascii_case("pk3")))
            .collect::<Vec<_>>();
        Self::open_paths(base, paths)
    }

    fn open_paths(base: &Path, mut paths: Vec<PathBuf>) -> Result<Self> {
        paths.sort_by_key(|p| p.file_name().unwrap_or_default().to_ascii_lowercase());
        if paths.is_empty() {
            bail!("No PK3 archives found in {}", base.display());
        }
        let mut entries = BTreeMap::new();
        let mut packs = Vec::new();
        #[cfg(target_arch = "wasm32")]
        let (mut sizes, mut spans, mut windows) = (Vec::new(), Vec::new(), Vec::new());
        for path in &paths {
            #[cfg(not(target_arch = "wasm32"))]
            let mut pack = open_pack(path)
                .with_context(|| format!("Invalid PK3: {}", path.display()))?;
            #[cfg(target_arch = "wasm32")]
            let (mut pack, window) = open_pack(path)
                .with_context(|| format!("Invalid PK3: {}", path.display()))?;
            #[cfg(target_arch = "wasm32")]
            windows.push(window);
            let p = packs.len();
            #[cfg(target_arch = "wasm32")]
            let listed = web::central_directory(path)?.filter(|c| c.len() == pack.len());
            #[cfg(target_arch = "wasm32")]
            if let Some(central) = &listed {
                for (i, c) in central.iter().enumerate() {
                    if !c.name.ends_with('/') {
                        entries.insert(normalize(&c.name), (p, i));
                    }
                }
                sizes.push(central.iter().map(|c| (c.size, c.crc)).collect());
                spans.push(central.iter().map(|c| (c.header, c.span)).collect());
            }
            #[cfg(target_arch = "wasm32")]
            let listed = listed.is_some();
            #[cfg(not(target_arch = "wasm32"))]
            let listed = false;
            if !listed {
                #[cfg(target_arch = "wasm32")]
                let mut pack_sizes = Vec::new();
                for i in 0..pack.len() {
                    let entry = pack.by_index(i)?;
                    #[cfg(target_arch = "wasm32")]
                    pack_sizes.push((entry.size(), entry.crc32()));
                    if !entry.is_dir() {
                        entries.insert(normalize(entry.name()), (p, i));
                    }
                }
                #[cfg(target_arch = "wasm32")]
                sizes.push(pack_sizes);
                #[cfg(target_arch = "wasm32")]
                spans.push(Vec::new());
            }
            packs.push(pack);
        }
        Ok(Self {
            packs,
            paths,
            entries,
            base: base.to_path_buf(),
            #[cfg(target_arch = "wasm32")]
            sizes,
            #[cfg(target_arch = "wasm32")]
            spans,
            #[cfg(target_arch = "wasm32")]
            windows,
        })
    }

    /// Saves identify core data independently of all known language packs.
    /// Reconstruct older identities with and without English, using the same
    /// core packs and available translations. Runtime mounting is unchanged.
    pub(crate) fn save_fingerprints(&mut self) -> Result<(String, Vec<String>)> {
        let core = self
            .paths
            .iter()
            .filter(|p| !is_language_pack(p))
            .cloned()
            .collect::<Vec<_>>();
        let identity = Self::open_paths(&self.base, core.clone())?.fingerprint()?;
        let mut legacy = vec![self.fingerprint()?];
        let mut languages = self
            .paths
            .iter()
            .filter(|p| is_language_pack(p))
            .cloned()
            .collect::<Vec<_>>();
        if let Some(managed) = self.base.ancestors().find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case("game-data"))
        }) {
            if let Ok(entries) = fs::read_dir(managed.join("languages")) {
                languages.extend(
                    entries
                        .filter_map(|e| e.ok().map(|e| e.path()))
                        .filter(|p| is_language_pack(p)),
                );
            }
        }
        let mut bases = vec![core.clone()];
        for english in languages.iter().filter(|p| is_english_pack(p)) {
            let mut paths = core.clone();
            paths.push(english.clone());
            // Includes the previous save identity that kept English in the core.
            if let Ok(fingerprint) = Self::open_paths(&self.base, paths.clone())
                .and_then(|mut mounted| mounted.fingerprint())
            {
                legacy.push(fingerprint);
                bases.push(paths);
            }
        }
        for language in languages.iter().filter(|p| !is_english_pack(p)) {
            for base in &bases {
                let mut paths = base.clone();
                paths.push(language.clone());
                // An unreadable inactive cache must not prevent launching.
                if let Ok(fingerprint) = Self::open_paths(&self.base, paths)
                    .and_then(|mut mounted| mounted.fingerprint())
                {
                    legacy.push(fingerprint);
                }
            }
        }
        legacy.sort();
        legacy.dedup();
        Ok((identity, legacy))
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(&normalize(name))
    }
    pub fn pack_count(&self) -> usize {
        self.packs.len()
    }
    pub fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        let &(p, i) = self
            .entries
            .get(&normalize(name))
            .with_context(|| format!("Asset not found: {name}"))?;
        // One bridge read for the whole entry instead of buffered header/data reads.
        #[cfg(target_arch = "wasm32")]
        if let Some(&(offset, span)) = self.spans[p].get(i) {
            self.windows[p].prefetch(offset, span);
        }
        let data = self.read_entry(p, i, name);
        #[cfg(target_arch = "wasm32")]
        self.windows[p].clear();
        data
    }
    fn read_entry(&mut self, p: usize, i: usize, name: &str) -> Result<Vec<u8>> {
        let entry = self.packs[p].by_index(i)?;
        if entry.size() > MAX_ENTRY {
            bail!("Asset exceeds 128 MiB limit: {name}");
        }
        let mut data = Vec::with_capacity(entry.size() as usize);
        entry
            .take(MAX_ENTRY + 1)
            .read_to_end(&mut data)
            .with_context(|| format!("Reading {name}"))?;
        if data.len() as u64 > MAX_ENTRY {
            bail!("Expanded asset exceeds limit: {name}");
        }
        Ok(data)
    }
    pub fn maps(&self) -> Vec<String> {
        self.names()
            .filter(|n| n.starts_with("maps/") && n.ends_with(".bsp"))
            .map(|n| {
                n.trim_start_matches("maps/")
                    .trim_end_matches(".bsp")
                    .to_owned()
            })
            .collect()
    }
}
fn normalize(s: &str) -> String {
    s.replace('\\', "/").to_ascii_lowercase()
}

fn is_language_pack(path: &Path) -> bool {
    matches!(path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_ascii_lowercase().as_str(),
        "pak4_english.pk3" | "pak4_german.pk3" | "pak4_french.pk3" | "pak4_spanish.pk3" | "pak4_dutch.pk3" |
        "base_g.pk3" | "base_f.pk3" | "base_s.pk3" | "base_d.pk3")
}

fn is_english_pack(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case("pak4_english.pk3"))
}

/// Browser backend: bridges to web/worker.js (PK3 bytes, user data, page options).
#[cfg(target_arch = "wasm32")]
pub(crate) mod web {
    use std::{
        io::{self, Read},
        path::{Path, PathBuf},
    };

    extern "C" {
        fn lgvfs_fetch(url: *const u8, len: usize) -> i32;
        fn lgvfs_take(dst: *mut u8);
        fn lgweb_put(url: *const u8, len: usize, body: *const u8, body_len: usize) -> i32;
    }

    extern "C" {
        fn lgweb_fatal(text: *const u8, len: usize);
    }
    /// Shows a fatal error on the page (stderr and process exit do nothing in the browser).
    pub fn fatal(text: &str) {
        unsafe { lgweb_fatal(text.as_ptr(), text.len()) };
    }

    /// Blocking PUT against the local server; returns an error unless it answers 2xx.
    pub fn put(url: &str, body: &[u8]) -> io::Result<()> {
        let status = unsafe { lgweb_put(url.as_ptr(), url.len(), body.as_ptr(), body.len()) };
        if (200..300).contains(&status) {
            Ok(())
        } else {
            Err(io::Error::other(format!("PUT {url}: HTTP {status}")))
        }
    }

    #[no_mangle]
    pub extern "C" fn lgvfs_crate_version() -> u32 {
        1
    }

    /// Blocking GET against the local server.
    pub fn fetch(url: &str) -> io::Result<Vec<u8>> {
        let n = unsafe { lgvfs_fetch(url.as_ptr(), url.len()) };
        if n < 0 {
            return Err(io::Error::new(io::ErrorKind::NotFound, url.to_owned()));
        }
        let mut data = vec![0u8; n as usize];
        unsafe { lgvfs_take(data.as_mut_ptr()) };
        Ok(data)
    }

    /// Diagnostics for the page: simulated seconds, fixed ticks and the player's feet position.
    static DEBUG_SIM: std::sync::Mutex<[f64; 5]> = std::sync::Mutex::new([0.; 5]);
    pub fn debug_sim(elapsed: f64, ticks: u64, feet: macroquad::math::Vec3) {
        let mut d = DEBUG_SIM.lock().unwrap();
        *d = [d[0] + elapsed, ticks as f64, feet.x as f64, feet.y as f64, feet.z as f64];
    }
    #[no_mangle]
    pub extern "C" fn lgweb_debug_sim(index: u32) -> f64 {
        DEBUG_SIM.lock().unwrap().get(index as usize).copied().unwrap_or(0.)
    }

    /// Query string as command-line options: `?map=hedge1&fly` -> `--map hedge1 --fly`.
    pub fn query_args() -> Vec<String> {
        let query = fetch("lgweb:query").unwrap_or_default();
        let mut args = Vec::new();
        for pair in String::from_utf8_lossy(&query).split('&').filter(|p| !p.is_empty()) {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            args.push(format!("--{key}"));
            if !value.is_empty() {
                args.push(value.to_owned());
            }
        }
        args
    }

    /// Pack files offered by the worker (base/*.pk3), one name per line.
    pub fn pack_paths(base: &Path) -> io::Result<Vec<PathBuf>> {
        let list = fetch("lgweb:packs")?;
        Ok(String::from_utf8_lossy(&list)
            .lines()
            .filter(|n| !n.is_empty())
            .map(|n| base.join(n))
            .collect())
    }

    extern "C" {
        fn lgweb_pack_len(name: *const u8, len: usize) -> f64;
        fn lgweb_pack_read(name: *const u8, len: usize, offset: f64, dst: *mut u8, n: usize) -> i32;
    }

    /// Random-access view of one original PK3, read synchronously by web/worker.js
    /// (an OPFS copy on a static host, HTTP range requests against the local dev server).
    pub struct PackFile {
        name: String,
        len: u64,
        pos: u64,
        window: Window,
    }

    /// Bytes of one archive range fetched ahead of an entry read; reads inside it are served
    /// from memory. Shared between the archive's reader and `Assets`.
    #[derive(Clone)]
    pub struct Window {
        name: std::rc::Rc<str>,
        len: u64,
        data: std::rc::Rc<std::cell::RefCell<Option<(u64, Vec<u8>)>>>,
    }

    impl Window {
        pub fn prefetch(&self, offset: u64, span: u64) {
            let n = span.min(self.len.saturating_sub(offset)) as usize;
            let mut bytes = vec![0u8; n];
            let mut got = 0;
            while got < n {
                let read = unsafe {
                    lgweb_pack_read(
                        self.name.as_ptr(),
                        self.name.len(),
                        (offset + got as u64) as f64,
                        bytes[got..].as_mut_ptr(),
                        n - got,
                    )
                };
                if read <= 0 {
                    break;
                }
                got += read as usize;
            }
            bytes.truncate(got);
            *self.data.borrow_mut() = Some((offset, bytes));
        }
        pub fn clear(&self) {
            *self.data.borrow_mut() = None;
        }
        /// Copies from the window if `pos` lies inside it.
        fn serve(&self, pos: u64, buf: &mut [u8]) -> Option<usize> {
            let data = self.data.borrow();
            let (start, bytes) = data.as_ref()?;
            let at = pos.checked_sub(*start)? as usize;
            let available = bytes.get(at..)?;
            if available.is_empty() {
                return None;
            }
            let n = available.len().min(buf.len());
            buf[..n].copy_from_slice(&available[..n]);
            Some(n)
        }
    }

    impl PackFile {
        pub fn open(path: &Path) -> io::Result<Self> {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_owned();
            let len = unsafe { lgweb_pack_len(name.as_ptr(), name.len()) };
            if len < 0. {
                return Err(io::Error::new(io::ErrorKind::NotFound, name));
            }
            let window = Window {
                name: name.as_str().into(),
                len: len as u64,
                data: Default::default(),
            };
            Ok(Self {
                name,
                len: len as u64,
                pos: 0,
                window,
            })
        }
        pub fn window(&self) -> Window {
            self.window.clone()
        }
    }

    impl Read for PackFile {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = buf.len().min(self.len.saturating_sub(self.pos) as usize);
            if n == 0 {
                return Ok(0);
            }
            if let Some(served) = self.window.serve(self.pos, &mut buf[..n]) {
                self.pos += served as u64;
                return Ok(served);
            }
            let got = unsafe {
                lgweb_pack_read(
                    self.name.as_ptr(),
                    self.name.len(),
                    self.pos as f64,
                    buf.as_mut_ptr(),
                    n,
                )
            };
            if got <= 0 {
                return Err(io::Error::other(format!("reading {}", self.name)));
            }
            self.pos += got as u64;
            Ok(got as usize)
        }
    }

    pub struct CentralEntry {
        pub name: String,
        pub size: u64,
        pub crc: u32,
        /// Local header offset and an upper bound of the bytes up to the end of its data.
        pub header: u64,
        pub span: u64,
    }

    /// Names, sizes and CRCs in central-directory order (the zip crate's index order), read
    /// with two bridge calls. `None` for anything this plain reader does not handle (ZIP64,
    /// archive comments, truncation); the caller then falls back to `by_index`.
    pub fn central_directory(path: &Path) -> io::Result<Option<Vec<CentralEntry>>> {
        let mut file = PackFile::open(path)?;
        let tail_len = file.len.min(22 + 65535);
        let mut tail = vec![0u8; tail_len as usize];
        let tail_start = file.len - tail_len;
        io::Seek::seek(&mut file, io::SeekFrom::Start(tail_start))?;
        file.read_exact(&mut tail)?;
        let u16_at = |b: &[u8], o: usize| u16::from_le_bytes([b[o], b[o + 1]]) as usize;
        let u32_at = |b: &[u8], o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let Some(eocd) = (0..=tail.len().saturating_sub(22))
            .rev()
            .find(|&i| u32_at(&tail, i) == 0x0605_4b50)
        else {
            return Ok(None);
        };
        let count = u16_at(&tail, eocd + 10);
        let (size, offset) = (u32_at(&tail, eocd + 12) as u64, u32_at(&tail, eocd + 16) as u64);
        if count == 0xffff || offset == 0xffff_ffff || offset + size > file.len {
            return Ok(None);
        }
        let mut cd = vec![0u8; size as usize];
        io::Seek::seek(&mut file, io::SeekFrom::Start(offset))?;
        file.read_exact(&mut cd)?;
        let mut entries = Vec::with_capacity(count);
        let mut p = 0;
        for _ in 0..count {
            if p + 46 > cd.len() || u32_at(&cd, p) != 0x0201_4b50 {
                return Ok(None);
            }
            let (name_len, extra_len, comment_len) = (u16_at(&cd, p + 28), u16_at(&cd, p + 30), u16_at(&cd, p + 32));
            let Some(name) = cd.get(p + 46..p + 46 + name_len) else {
                return Ok(None);
            };
            if !name.is_ascii() {
                return Ok(None); // non-ASCII names: let the zip crate decode them
            }
            let header = u32_at(&cd, p + 42) as u64;
            // The local extra field may differ from the central one; allow generous slack.
            let span = (30 + name_len + extra_len + 1024) as u64 + u32_at(&cd, p + 20) as u64;
            entries.push(CentralEntry {
                name: String::from_utf8_lossy(name).into_owned(),
                size: u32_at(&cd, p + 24) as u64,
                crc: u32_at(&cd, p + 16),
                header,
                span: span.min(file.len.saturating_sub(header)),
            });
            p += 46 + name_len + extra_len + comment_len;
        }
        Ok(Some(entries))
    }

    impl io::Seek for PackFile {
        fn seek(&mut self, from: io::SeekFrom) -> io::Result<u64> {
            let target = match from {
                io::SeekFrom::Start(p) => p as i128,
                io::SeekFrom::End(d) => self.len as i128 + d as i128,
                io::SeekFrom::Current(d) => self.pos as i128 + d as i128,
            };
            if target < 0 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek before start"));
            }
            self.pos = target as u64;
            Ok(self.pos)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn language_switch_preserves_save_identity_and_alphabetical_precedence() -> Result<()> {
        let root = std::env::temp_dir().join(format!("looking-glass-language-{}", std::process::id()));
        let base = root.join("game-data/base");
        let cache = root.join("game-data/languages");
        fs::create_dir_all(&base).context("create test base")?;
        fs::create_dir_all(&cache).context("create test cache")?;
        let write = |path: &Path, entry: &str, text: &[u8]| -> Result<()> {
            let mut z = zip::ZipWriter::new(File::create(path)?);
            z.start_file(entry, zip::write::FileOptions::default())?;
            z.write_all(text)?;
            z.finish()?;
            Ok(())
        };
        write(&base.join("pak0.pk3"), "ui/main.urc", b"English")?;
        let core_id = Assets::open(&base)?.fingerprint()?;
        write(&base.join("pak4_german.pk3"), "ui/main.urc", b"German")?;
        let mut german = Assets::open(&base)?;
        let old_german = german.fingerprint()?;
        assert_eq!(german.read("ui/main.urc")?, b"German");
        assert_eq!(german.save_fingerprints()?.0, core_id);
        drop(german);
        fs::copy(base.join("pak4_german.pk3"), cache.join("pak4_german.pk3")).context("cache German test pack")?;
        fs::remove_file(base.join("pak4_german.pk3"))?;
        write(&base.join("pak4_french.pk3"), "ui/main.urc", b"French")?;
        let mut french = Assets::open(&base)?;
        let (id, legacy) = french.save_fingerprints()?;
        assert_eq!(id, core_id);
        assert!(legacy.contains(&old_german));
        assert_eq!(french.read("ui/main.urc")?, b"French");
        drop(french);
        write(&base.join("pak5_mod.pk3"), "ui/main.urc", b"Mod wins")?;
        let mut modified = Assets::open(&base)?;
        assert_eq!(modified.read("ui/main.urc")?, b"Mod wins");
        let (changed, legacy) = modified.save_fingerprints()?;
        assert_ne!(changed, core_id);
        assert!(!legacy.contains(&old_german));
        drop(modified);
        for path in [base.join("pak0.pk3"), base.join("pak4_french.pk3"), base.join("pak5_mod.pk3"), cache.join("pak4_german.pk3")] { fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?; }
        for path in [base, cache, root.join("game-data"), root] { fs::remove_dir(&path).with_context(|| format!("remove directory {}", path.display()))?; }
        Ok(())
    }
    #[test]
    fn later_pack_wins_and_paths_are_case_insensitive() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("looking-glass-vfs-{}", std::process::id()));
        fs::create_dir_all(&dir)?;
        for (name, value) in [("pak0.pk3", b"old"), ("pak5.pk3", b"new")] {
            let mut z = zip::ZipWriter::new(File::create(dir.join(name))?);
            z.start_file("Textures/Test.ftx", zip::write::FileOptions::default())?;
            z.write_all(value)?;
            z.finish()?;
        }
        let mut a = Assets::open(&dir)?;
        assert_eq!(a.read("TEXTURES\\test.ftx")?, b"new");
        assert!(a.read("missing").is_err());
        drop(a);
        for name in ["pak0.pk3", "pak5.pk3"] {
            fs::remove_file(dir.join(name))?;
        }
        fs::remove_dir(dir)?;
        Ok(())
    }
}
