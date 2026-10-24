# Fizibilite denemesi (c): Looking Glass → WebAssembly

Tarih: 2026-10-23. Süre: yaklaşık yarım iş günü (bütçe 5 gündü).
Taban: Looking Glass `055b1d1` (MIT), `port/lookingglass/` altında. Yerel değişiklikler ayrı commit'lerde.

## Sonuç: 4 başarı ölçütünün 4'ü de geçti

| # | Ölçüt | Sonuç | Kanıt |
| --- | --- | --- | --- |
| 1 | Rust kurulur, LG yerelde derlenip açılır | ✅ rustc 1.99.1 (MSVC). `cargo build --release` 4,5 dk. `--map hedge1 --capture` orijinal pk3'lerden render ediyor | yerel ekran görüntüsü (scratchpad) |
| 2 | `wasm32` derlemesi Chrome'da ana menüyü gösterir | ✅ wasm 17 MB, menü 240 fps | `tools/browser-check.mjs` |
| 3 | İlk seviye, orijinal pk3'lerden sunucu üzerinden (çıkarmadan) yüklenir ve ≥30 fps çalışır | ✅ New Game → açılış sahnesi → oynanış. Alice yürüyor | aşağıdaki tablo |
| 4 | Sekme belleği < 2 GB | ✅ en fazla ~230 MB (wasm 208 MB + JS yığını ~20 MB) | aşağıdaki tablo |

### Ölçümler (headless Chrome, ANGLE D3D11, RTX 3060 Ti, `simd128` açık)

| Harita | fps (wasm) | wasm belleği | Yükleme: istek / bayt / bloklama süresi |
| --- | ---: | ---: | --- |
| menü | 240 | 19 MB | 270 / 21 MB / 1,0 s |
| kampanya başlangıcı (açılış sahnesi + ilk seviye) | 86–110 | 208 MB | 2.915 / 210 MB / 8,0 s |
| hedge1 | 136 | 99 MB | 2.280 / 95 MB / 6,0 s |
| hedge3 (en büyük BSP) | 47 (SIMD'siz 39; **yerelde ~86**) | 150 MB | 2.494 / 105 MB / 5,4 s |
| garden2 | 81 (SIMD'siz 71) | 85 MB | 2.500 / 113 MB / 5,6 s |
| utemple | 157 | 93 MB | 2.197 / 94 MB / 5,2 s |
| pandemonium | 106 | 75 MB | 2.701 / 118 MB / 6,2 s |

Headless ölçümler tarayıcının normal penceresindeki değerlerden düşük olabilir. hedge3'te
wasm yerelden yaklaşık 2 kat yavaş (çıkarım: CPU'ya bağlı iş yükü).

## Yapılan değişiklikler

1. **`LoopCue.id` → `u64`**: Ses döngüsü kimlikleri kategoriyi 32. bitin üstünde kodluyor
   (`0x4_0000_0000usize`). wasm32'de `usize` 32 bit olduğu için derlenmiyordu.
2. **wasm `Assets` arka ucu** (`src/assets.rs`, `mod web`): Aynı API (`by_index`, `name`,
   `size`, `crc32`, `Read`) korundu. İndeks `/vfs-index`'ten bir kez alınıyor, girdiler
   `/vfs/<pack>/<index>`'ten senkron XHR ile isteniyor. Önemli gözlem: önceden wasm32'de
   `fs::read_dir` her zaman hata döndürdüğü için derleyici bütün oyunu ölü kod olarak atıyordu
   (wasm 143 KB). Arka uç eklenince wasm 17 MB oldu.
3. **`.cargo/config.toml`**: `--allow-undefined` (miniquad'ın JS import'ları; yeni rustc
   bunlara varsayılan olarak izin vermiyor) ve `+simd128`.
4. **`main.rs`**: wasm'de komut satırı argümanları URL sorgusundan geliyor
   (`?map=hedge1&fly` → `--map hedge1 --fly`).
5. **JS** (`web/`): macroquad'ın paketi miniquad 0.4.8'den eski olduğu için çöktü
   (`glCheckFramebufferStatus` yok). miniquad 0.4.8'in kendi `gl.js`'si kullanıldı. `lgweb.js`
   eklentisi: VFS köprüsü, `glDepthMask`, pointer lock'u yalnızca tıklamada isteme (LG her karede
   istiyordu), F1–F12/Tab/Space tuşlarının tarayıcı davranışını engelleme (F5 hızlı kayıt
   sayfayı yenilemesin diye).
6. **Sunucu** (`server/server.mjs`): yalnızca 127.0.0.1. `config.json` → `gameBase`.
   LG'nin bağlama kuralıyla aynı (yalnızca `base/*.pk3`, alfabetik, sonraki kazanır).

Yerel (Windows) derleme bu değişikliklerle hâlâ derleniyor.

## Bilinen eksikler (sonraki işler)

| Konu | Durum | Plan |
| --- | --- | --- |
| **Ses ve müzik** | Tarayıcıda sessiz. rodio/cpal wasm'de aygıt bulamıyor ve LG zarif şekilde sessiz moda geçiyor | wasm `Playback`: rodio'nun karıştırdığı örnekler bir AudioWorklet halka tamponuna aktarılır (karıştırma Rust'ta kalır) |
| **Kayıt/yükleme ve ayarlar** | `std::fs` wasm'de çalışmıyor, kayıtlar kalıcı değil (çıkarım; henüz denenmedi) | Sunucuya `/userdata/*` (gitignore'lu `userdata/` klasörü) ve senkron okuma/yazma |
| Yükleme donması | Seviye başına 5–8 s ana thread bloklanıyor (senkron XHR) | Yükleme ekranı, sunucu tarafında harita başına ön yükleme veya önbellek |
| Kampanya boyunca bellek | Tek seviyede ≤ 210 MB. Uzun oturumda seviye geçişleri test edilmedi | 39 ziyaretlik zincir testi |
| RoQ videolar | LG'nin kendi davranışına bağlı, denenmedi | — |
| Performans | En kötü harita 47 fps (headless) | Profil çıkarma. Gerekirse WebGL2 / instancing |
| Upstream takibi | Yama küçük (3 dosya + mekanik `as u64`) | Upstream'deki yeni sürümler rebase ile alınır |

## Nasıl test edilir

```sh
# bir kez: Rust ve wasm hedefi (kuruldu), config.json
cp config.example.json config.json        # gameBase: ../Alice1/bin/base
npm run build:wasm                        # ~2,5 dk
npm run dev                               # http://127.0.0.1:8080/
```

- `http://127.0.0.1:8080/`: ana menü → **New Game** (açılış sahnesi; **Enter'ı basılı tut** = atla).
- `http://127.0.0.1:8080/?map=hedge1`: doğrudan bir haritaya. Diğer LG seçenekleri de çalışır
  (ör. `?map=pandemonium&fly`).
- Fareyi yakalamak için oyun alanına **bir kez tıkla** (Esc bırakır). WASD + fare, Space zıplama.
- Sol alttaki yeşil satır: fps, wasm belleği, VFS istek sayısı.
- Otomatik test: `node tools/browser-check.mjs "http://127.0.0.1:8080/?map=hedge1" shot.png 30`
