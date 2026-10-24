//! User data (saves, preferences, audio settings). Native builds use the local
//! filesystem; the browser build stores the same relative paths through the
//! alice-web server (`/userdata/<path>`), which keeps them outside the game data.
use std::{io, path::Path};

#[cfg(not(target_arch = "wasm32"))]
pub fn read(path: &Path) -> io::Result<Vec<u8>> {
    std::fs::read(path)
}
#[cfg(not(target_arch = "wasm32"))]
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    std::fs::write(path, bytes)
}
#[cfg(not(target_arch = "wasm32"))]
pub fn create_dir_all(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)
}
#[cfg(not(target_arch = "wasm32"))]
pub fn is_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(target_arch = "wasm32")]
fn url(path: &Path) -> io::Result<String> {
    let rel = path.to_string_lossy().replace('\\', "/");
    if rel.starts_with('/') || rel.contains(':') || rel.split('/').any(|p| p == "..") {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, rel));
    }
    Ok(format!("/userdata/{rel}"))
}
#[cfg(target_arch = "wasm32")]
pub fn read(path: &Path) -> io::Result<Vec<u8>> {
    crate::assets::web::fetch(&url(path)?)
}
#[cfg(target_arch = "wasm32")]
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    // The server writes to a temporary file and renames it, so the update is atomic.
    crate::assets::web::put(&url(path)?, bytes)
}
#[cfg(target_arch = "wasm32")]
pub fn create_dir_all(_path: &Path) -> io::Result<()> {
    Ok(())
}
#[cfg(target_arch = "wasm32")]
pub fn is_file(path: &Path) -> bool {
    url(path).is_ok_and(|u| crate::assets::web::fetch(&u).is_ok())
}

/// Wall-clock milliseconds since the Unix epoch (`SystemTime::now` panics on wasm32).
#[cfg(not(target_arch = "wasm32"))]
pub fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
#[cfg(target_arch = "wasm32")]
pub fn unix_millis() -> u64 {
    (macroquad::miniquad::date::now() * 1000.) as u64
}
