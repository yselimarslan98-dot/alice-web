# Aşama 0 — Keşif bulguları

Tarih: 2026-10-23. Bu notlar **gözlem** (dosya baytlarından doğrudan okunan), **çıkarım**
(gözleme dayanan yorum) ve **bilinmeyen** ayrımıyla yazılmıştır. Orijinal oyun dosyalarına
yalnızca okuma yapıldı; hiçbir dosya değiştirilmedi veya depoya kopyalanmadı.

## Yeniden üretme

```sh
node tools/pk3-inventory.mjs ../Alice1/bin/base --json /tmp/inv.json   # tüm pk3 envanteri + başlık örnekleri
node tools/pe-scan.mjs ../Alice1/bin/fgamex86.dll exports             # export tablosu
node tools/pe-scan.mjs ../Alice1/bin/fgamex86.dll imports
node tools/pe-scan.mjs ../Alice1/bin/fgamex86.dll strings 6 'regex'   # ASCII string araması
```

Kanıt dosyaları:

- `docs/evidence/pk3-inventory-summary.json`: her pk3 için uzantı ve klasör sayımı, başlık dağılımı.
- `docs/evidence/phase0-rea-bundle.json.gz`: REA Evidence paketi. pak0.pk3 envanteri ve üç PE için kaynak incelemesi.

### REA kanıt kimlikleri

| Konu | Evidence ID | Not |
| --- | --- | --- |
| pak0.pk3 envanteri (7.724 occurrence; 7.144 dosya + 580 dizin, bütünlük çelişkisi 0) | `ev_acfd9e4be2e5a9b747816936832a693043ab7f7c6e88a27617897f6521d8a627` (özet `ev_7c21fe6f…1237`) | sha256 `6a63b59c…da62` |
| pak5_mod.pk3 envanteri (31 occurrence) | `ev_7adeaec3040e1751c6bc20d078a8a6d123b385521bf11e2517f19435fb78009d` | sha256 `cc36d55e…2340` |
| alice.exe PE kaynakları (icon, string, version yok, manifest) | `ev_ee71982f4295c7700aeb00b13de19c09cd646ffb4596dae147eb1b6b50d920da` | sha256 `08bcbe4b…6e50` |
| fgamex86.dll PE kaynakları (yalnız manifest) | `ev_53c403aa97f8d6cccb1cbd44a1da319b8d6acf9a0ebc1369e638c87c16263512` | sha256 `2ead219a…b5b5` |
| cgamex86.dll PE kaynakları (yalnız manifest) | `ev_856e339dc8609cfc6b2be0ef03759d7e21db2d9cb8fb8f25d8638a6511efcd95` | sha256 `61c20f6a…743b` |

**REA sınırlaması (gözlem):** Bu makinede derin analiz sağlayıcısı yok. `open_binary`
fgamex86.dll'yi açıyor, ama `binary_overview`/decompile `provider_unavailable` döndürüyor:
Ghidra kurulu değil (`GHIDRA_INSTALL_DIR` tanımsız), PATH'te yalnızca Java 8 var, Hopper
yalnızca macOS'ta çalışıyor, IDA ve Kuna yapılandırılmamış. Bu yüzden export, import ve
string'ler `tools/pe-scan.mjs` ile ham PE baytlarından okundu. Fonksiyon gövdeleri, yapı
düzenleri ve `game_export_t` alan sırası **bilinmiyor**. Bunlar için Ghidra 12.x + JDK 21
kurulumu gerekiyor (A5 öncesi).

## 1. İkili dosyalar

| Dosya | PE makine | Zaman damgası | Gözlem |
| --- | --- | --- | --- |
| `alice.exe` | 0x14C (x86) | 2011-05-05 | Motor. Derleme string'i `May  5 2011`. 6 export: `CDAPFN0506_CL_InitInput`, `_CM_ClearLevelPatches`, `_CMod_LoadLeafSurfaces`, `_MUSIC_NewSoundtrack`, `_ROQ_GenYUVTables`, `_S_MILES_StopAllSounds`. Import: WINMM, WSOCK32, USER32, GDI32, ADVAPI32, SHELL32, XINPUT1_3 (OpenGL dinamik yükleniyor). |
| `fgamex86.dll` | 0x14C | 2011-05-05 | **Windows'ta yüklenen oyun mantığı.** Tek export: `GetGameAPI` @ `0x100754a0`. |
| `cgamex86.dll` | 0x14C | 2011-05-05 | İstemci oyun modülü. Tek export: `GetCGameAPI` @ `0x3001dec0`. |
| `fgame.dll`, `cgame.dll` | **0x1F2** | 2011-04-29 | x86 **değil**: COFF 0x1F2 PowerPC big-endian (çıkarım: 2011 Xbox 360 yan ürününden kalan derleme). REA bunları açmayı reddediyor ("unsupported PE architecture"). Analizde kullanılmamalı. |
| `lib/renderer.lib` | — | — | İncelenmedi (bilinmeyen). |
| `bin/SndDrivers/*` | — | — | Miles Sound System (mss32.dll, mp3dec.asi). Web'de WebAudio ile değiştirilecek. |

Çıkarım: `CDAPFN0506_*` export'ları motor fonksiyonlarına kopya koruma veya bütünlük
kancası gibi görünüyor. Web portu için önemi yok.

### Motor ve oyun arayüzü (string kanıtı)

- `alice.exe`: `updatefakk2.ritual.com`, `masterfakk2.ritual.com`, `fakkmaster.ritual.com`.
  Çıkarım: motor, Ritual'ın FAKK2 motorudur (id Tech 3 türevi). Arayüz sürümü kontrolü
  `game is version %i, not %i` mesajıyla yapılıyor. BSP sürüm kontrolü:
  `CM_LoadMap: %s has wrong version number (%i should be %i)`.
- Modeller (`alice.exe`): `TIKI_LoadAnimTan`, `TIKI_LoadAnimSkel`, `TIKI_LoadSkel ... (Supported: %i - %i)`.
  TIKI sistemi motor tarafında.
- fgamex86 ve cgamex86 içinde `fakk-base-1` geçiyor. fgame içinde ayrıca `gameversion`, `Alice Archive Version 1`,
  `Savegame from version %d of Alice`, `Check http://www.rogue-ent.com for an update`.
- **Script sistemi (fgamex86):** Tipli bir derleyici ve bytecode yorumlayıcısı var: `Compile failed.`,
  `Built-in functions cannot be called as threads`, `type mismatch on parm %i`,
  `Type mismatch on line %d. Func %s expected %s, got %s`, `Bad opcode %i`,
  `EnterFuncton: locals stack overflow`, `Program::GetFreeString`, RTTI `.?AVInterpreter@@`.
  Çıkarım: mesajlar QuakeC derleyicisi (qcc) ailesine çok benziyor. `.scr` dosyaları
  yükleme anında bytecode'a derleniyor.
- FAKK2 SDK karşılaştırması (gözlem): SDK'nın `fgame/` klasöründe `script.cpp`,
  `scriptmaster.cpp`, `scriptslave.cpp` var, ama yukarıdaki derleyici string'leri **yok**.
  Yani Alice'in script dili FAKK2'den farklı, sonradan eklenmiş bir dil.
  SDK `GAME_API_VERSION 4`. Alice'in değeri bilinmiyor (decompile gerekiyor).
- Olay ve komut sistemi: fgame içinde komut adı + açıklama çiftleri var
  (ör. `setidlethread` / "Sets the thread that will be run if this actor gets back to the idle state again.").
  Bu, FAKK2'nin `Event` dokümantasyon tablosu deseni. Toplam 8.319 ASCII string (≥6 karakter).

## 2. Veri arşivleri (.pk3 = ZIP, deflate/store)

| Arşiv | Girdi | İçerik |
| --- | ---: | --- |
| `pak0.pk3` (289 MB) | 7.723 | 36 BSP, modeller (.tik/.skb/.ska/.tan/.cik), 1.896 wav, script, shader, UI, AI, kamera yolları |
| `pak1_large.pk3` (158 MB) | 3.368 | 2.944 `.ftx` doku (309 MB açılmış) |
| `pak2.pk3` (102 MB) | 71 | 29 müzik mp3 + 38 `.mus` müzik tanımı |
| `pak3.pk3` (93 MB) | 9 | 4 RoQ video + 4 mp3 ses izi |
| `pak4_english.pk3` | 44 | 43 `dialog/*.tlk` altyazı. **İçerik Türkçe** (Kasım 2011, çeviri yaması) |
| `pak5_mod.pk3` (18 MB) | 30 | Karakter dokularının `.ftx` yenileri (override) |
| `loc/base_{d,f,g,s}.pk3` | 88–1.218 | Dil paketleri. base_f'nin 1.145 girdisinden 924'ü pak0'daki dosyaları geçersiz kılıyor (wav, lip, tga, urc) |

Toplam açılmış veri yaklaşık 1,1 GB. Bütün `.bsp` ve `.ftx` girdileri aşağıdaki yapılarla tutarlı.

### Format tablosu

| Uzantı | Sayı | Yapı (gözlem) | Web karşılığı |
| --- | ---: | --- | --- |
| `.bsp` | 36 | `FAKK` + sürüm **42** + 4 baytlık sağlama değeri. 12. bayttan itibaren **20 lump** (offset, uzunluk). Başlık 172 bayt; son lump dosya sonuna tam oturuyor. Lump'lar aşağıda. | Kendi ayrıştırıcımız |
| `.ftx` | 2.974 | 3×u32 (genişlik, yükseklik, bayrak 0/1) + RGBA8, uzunluk = 12+w·h·4 | Doğrudan WebGL dokusu |
| `.tga` | 284+ | tip 2 (24/32 bpp) ve tip 10 (RLE). Lightmap dışı 2B ve UI | JS TGA çözücü |
| `.tik` | 800 | Metin `TIKI`: `setup { scale, path, surface ... shader }`, animasyonlar, `init`/frame komutları | Ayrıştırıcı + olay yorumlayıcı |
| `.cik` | 797 | İkili, önbelleğe alınmış TIKI (yol + sınırlar). Çıkarım: motorun ürettiği önbellek | Yok sayılabilir |
| `.skb` | 79 | `SKL ` sürüm 3 (64) / 2 (15). İskeletli mesh | Ayrıştırıcı |
| `.ska` | 1.402 | `SKAN` sürüm 3 (1.298) / 2 (104). İskelet animasyonu | Ayrıştırıcı |
| `.tan` | 694 | `TAN ` sürüm 2. Vertex animasyonlu mesh (MD3 benzeri) | Ayrıştırıcı |
| `.shader` | 52 | Quake 3 shader sözdizimi (`map`, `blendfunc`, `tcmod`, `rgbGen`, `surfaceparm`, `deformVertexes`) | Q3 shader → GLSL |
| `.scr` | 73 | C benzeri script: `#include`, `void f(string s, float t)`, `entity e = getentity(..)`, `e.moveUp(8)`, `wait`, `waitfor`, `thread`, `while`, `if`, `for`. Toplam 864 KB | Derleyici + VM (TS) |
| `.st` | 78 | AI ve oyuncu durum makinesi: `state X { legs { anim : koşul } behavior .. states { HEDEF : KOŞUL ... } }` | Yorumlayıcı |
| `.urc` | 12+24 | Menü tanımı: `menu "main" 640 480`, `resource Label { rect, shader, ... }` | DOM veya canvas UI |
| `.snd` | 36 | Konsol komut satırları: `spawn TriggerSpeaker origin ... sound ...` | Ayrıştırıcı |
| `.cam` | 396 | `spawn SplinePath targetname .. origin .. angles .. speed` | Ayrıştırıcı |
| `.pth` | 35 | İkili "ALIC Alice Archive Version 1": `PathSearch`/`PathNode` (navigasyon önbelleği) | Ayrıştırıcı veya `info_pathnode`'lardan yeniden üretme |
| `.lip` | 366+ | Metin: süre + 0–255 ağız açıklığı örnekleri | Doğrudan |
| `.tlk` | 43 | `TALK`, ardından `"wav yolu"` + altyazı satırı | Doğrudan |
| `.mus` | 38 | `path ...`, `normal track.mp3`, `!normal loop/fadetime/volume` | Doğrudan |
| `.ritualfont` | 14 | `RitFont`: yükseklik, en-boy oranı, karakter indirection tablosu, ardından glif konumları | Ayrıştırıcı |
| `.wav` | 1.896+ | PCM 16-bit mono, 22.050 Hz (13 dosya 44.100 Hz) | WebAudio `decodeAudioData` |
| `.mp3` | 29+ | Müzik ve video sesi | WebAudio veya `<audio>` |
| `.roq` | 4 | id RoQ video (ealogo, opening vb.) | JS RoQ çözücü (veya atla) |

### FAKK v42 BSP lump'ları (36 haritanın hepsinde kayıt boyutu ile bölünebilirlik doğrulandı)

| # | İçerik | Kayıt | Doğrulama |
| ---: | --- | ---: | --- |
| 0 | shaders: ad[64], surfaceFlags, contents, subdivisions | 76 | ad okunuyor (`textures/floor/concretemoss`) |
| 1 | planes | 16 | |
| 2 | lightmaps 128×128 RGB | 49.152 | |
| 3 | surfaces (Q3 104 bayt + subdivisions) | 108 | patch: tip 2, 5×3 = 15 vertex |
| 4 | drawverts xyz, st, lm, normal, rgba | 44 | normal birim vektör |
| 5 | indexes | 4 | |
| 6 | leafbrushes | 4 | |
| 7 | leafsurfaces | 4 | |
| 8 | leafs | 48 | |
| 9 | nodes | 36 | |
| 10 | brushsides | 8 | |
| 11 | brushes | 12 | |
| 12 | fogs | 72 | |
| 13 | models | 40 | |
| 14 | entities (metin) | — | `{ "classname" ... }` |
| 15 | visibility | — | |
| 16–19 | lightgrid, entlights, entlightsvis, lightdefs | — | çıkarım (FAKK2 düzeni). Ayrıntılar bilinmiyor |

Çıkarım: ilk 15 lump Quake 3 düzenine birebir uyuyor. Bu da A1'de mevcut Q3 BSP
render tekniklerini (patch tesselation, lightmap, PVS) doğrudan kullanmamızı sağlıyor.

### İçerik ölçeği (iş yükü tahmini için)

- 36 haritada **23.241 entity** ve **252 farklı classname** var. En sık geçenler: `light` (8.908,
  render'da kullanılmıyor), `info_pathnode` (5.365), `info_splinepath` (1.724),
  `script_object` (1.053), `func_spawn`, `trigger_once`, `script_model`, `trigger_push`,
  `func_rotatingdoor`, `func_camera`... Düşman ve karakter sınıfları (`Enemies_*`,
  `Characters_*`, `Boss_*`) yaklaşık 70 türde.
- Script'lerde **1.607 farklı komut/fonksiyon** çağrısı var. En sık 20'si tüm kullanımın
  büyük kısmını oluşturuyor: `wait`, `anim`, `time`, `waitfor`, `nottriggerable`, `show`, `remove`,
  `notsolid`, `hide`, `trigger`, `movedown`, `bind`, `cuecamera`, `follow`, `cam_fadein/out`,
  `playdialog`, `playsound`, `warpto`...

## 3. Açık kaynak projeler

| Proje | Lisans (gözlem: GitHub API / dosya) | Ne işe yarar | Sınır |
| --- | --- | --- | --- |
| **Looking Glass** (`looking-glass-project/LookingGlass`, son commit `055b1d1`, 2026-10-21) | Motor kodu **MIT** (`LICENSE-MIT`), ayrıca `LEGAL.md` ve `THIRD_PARTY_NOTICES.txt` | Alice 2011 verisini doğrudan pk3'ten okuyan **Rust** motoru. README'ye göre 39 kampanya ziyaretinin hepsi oynanabilir. ~208 bin satır, 466 `.rs`. macroquad 0.4.14 + rodio + zip. Shader'lar GLSL `#version 100` (WebGL1 uyumlu). `docs/` klasöründe format notları ve seviye başına davranış belgeleri var | Yalnız Windows x64. **Wasm hedefi yok.** 326 noktada senkron pk3 okuma, 100 dosyada `std::fs` (çoğu test ve doğrulama), rodio (web'de cpal/wasm-bindgen gerekir). **Orijinal script'leri bilerek çalıştırmıyor:** her seviye elle yazılmış Rust denetleyicisi (`src/levels` 87 bin satır). Yapay zekâ destekli ve deneysel |
| **ioquake3** (`ioquake/ioq3`) | GPL-2.0 | Q3 motoru. README'de **resmi Emscripten desteği** var | FAKK formatlarını (TIKI, FTX, BSP42) bilmiyor |
| **OpenMoHAA** (`openmoh/openmohaa`) | GPL-2.0 | ioq3 + F.A.K.K SDK tabanlı MOHAA motoru. TIKI/skeletor kodu var (`code/corepp/tiki*`), `cmake/platforms/emscripten.cmake` mevcut | MOHAA BSP sürümü ve lump düzeni farklı. Script dili MOHAA ScriptMaster (Alice'in C benzeri dili değil). fgame MOHAA'ya özgü. FAKK/Alice desteği yok (ağaçta `fakk` eşleşmesi 0) |
| **F.A.K.K2 SDK** (`code.idtech.space/ritual/fakk2-sdk`) | **Ritual EULA**: kişisel, ticari olmayan kullanım. Açık kaynak değil | Alice fgame'inin atası (actor, behavior, characterstate, doors, camera...). `.st` durum makinesi buradan geliyor | Script derleyicisi yok. Kod kopyalamak lisans açısından riskli; yalnız davranış referansı |
| **QuakeJS** (`inolen/quakejs`) | Lisans belirtilmemiş | ioq3'ün eski Emscripten portu | Bakımı bitmiş |
| **ioquake3.js** (`klaussilveira/ioquake3.js`) | GPL-2.0 | Emscripten portu | 2015'ten beri güncellenmiyor |
| **webgl-quake3** (`toji/webgl-quake3`) | **Lisans dosyası yok** (kod kopyalanamaz) | WebGL Q3 BSP + shader renderer | Yalnız fikir referansı |
| three.js | MIT | Genel WebGL motoru | BSP ve Q3 shader desteği yok, kendimiz yazacağız |

## 4. Açık sorular / bilinmeyenler

- `game_export_t` / `game_import_t` alan sırası, Alice'in `GAME_API_VERSION` değeri (Ghidra gerekiyor).
- Script bytecode opcode seti. Kaynaktan derleyeceğimiz için zorunlu değil, ama
  built-in fonksiyonların **anlamı** fgame'den çıkarılmalı.
- `.st` koşul anahtar kelimelerinin tam listesi (fgame string'lerinden çıkarılabilir).
- Lump 16–19'un yapısı (lightgrid, statik model ışığı).
- `.skb` v2/v3 ve `.ska` v2/v3 ayrıntılı düzenleri (A3'te. Looking Glass `docs/CHARACTER.md` MIT referansı olarak kullanılabilir).
- pk3 öncelik sırası (alfabetik + `loc/` dili). Motorun arama yolu kuralı decompile ile doğrulanmalı.
