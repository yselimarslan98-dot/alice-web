# Port durumu (Looking Glass → tarayıcı)

Son güncelleme: 2026-10-24. Başlangıç denemesi: [SPIKE-C.md](SPIKE-C.md). Yayınlama: [PUBLISHING.md](PUBLISHING.md).

## Mimari

```
sayfa (main.js)                             Worker (worker.js)
  kurulum: klasör seç → OPFS'e kopyala         miniquad gl.js (DOM taklitleriyle)
  girdi → postMessage ──────────────────────▶  Looking Glass wasm (OffscreenCanvas, WebGL2)
  pointer lock (ham fare)                      pk3 okuma: zip crate ↔ köprü
  WebAudio ◀── PCM blokları ────────────────    · statik site: OPFS senkron tutamakları
  HUD ◀── istatistik ──────────────────────    · yerel geliştirme: senkron XHR, HTTP range
                                               kayıtlar: bellekte + OPFS'e kalıcı yazım
```

Gerekçe: Chrome, sayfanın ana thread'indeki senkron XHR'yi Service Worker'a yönlendirmiyor
(denendi: asenkron `fetch` SW'den 200 alırken senkron XHR ağdan 404 aldı). LG'nin varlık
yükleyicisi senkron olduğu için oyun, senkron dosya okumasının mümkün olduğu bir Worker'a taşındı.

## Çalışanlar

| Özellik | Durum |
| --- | --- |
| Statik site: klasör seç → kopyala (661 MB) → oyna | ✅ ilk ziyarette ~9 s (headless) |
| İkinci ziyarette doğrudan başlama, kayıtların kalması | ✅ |
| Yerel geliştirme: pk3'leri kopyalamadan yerinde okuma | ✅ |
| Menü, açılış sahnesi, oynanış, seviye geçişleri | ✅ (önceki dayanıklılık testinde 39/39 ziyaret) |
| Ses ve müzik | ✅ |
| Hızlı kayıt / yükleme, tercihler | ✅ OPFS'te |
| Kayıtları dosyaya dışa / içe aktarma (Ayarlar paneli, kurulum ekranı; silmeden önce teklif edilir) | ✅ |
| Eski sunucu kayıtlarının (`userdata/`) aktarımı | ✅ yerel modda bir kez |
| Hata mesajları sayfada | ✅ |
| Orijinal hareket hızları (koşma 173, yürüme 51,7 u/s; varsayılan koşma) | ✅ `?fast-movement` eski hızlar |
| Ayarlar paneli: FPS sınırı (varsayılan sınırsız, `#fps=N`), çizim çözünürlüğü (otomatik ≤1080p, %100/75/50), istatistik kutusu (kapalı/FPS/ayrıntılı) | ✅ |
| Xbox / PS5 kontrolcüsü (Gamepad API → LG'nin XInput katmanı; PS tuş adları) | ✅ (sahte kontrolcüyle test edildi) |
| Ham fare girdisi (`unadjustedMovement`) | ✅ (gerçek tarayıcıda; headless'ta yok) |

## Ölçümler (headless Chrome, RTX 3060 Ti)

| | Sunucu tabanlı eski sürüm | Worker + yerel geliştirme | Worker + OPFS (statik site) |
| --- | --- | --- | --- |
| hedge1 yükleme | 2.458 istek, ~6 s donma | 2.582 okuma, 63 MB, 3,1 s (sayfa donmuyor) | 2.582 okuma, 63 MB, **< 0,1 s** |
| hedge1 fps (60 kilidi) | 82–136 (kilitsiz) | 60 | 60 |

Performans iyileştirmeleri (bu dönem):
- Duyulamayan döngüsel ortam seslerini park etmek: hedge3 ses karıştırma 322 → 72 ms/s, 19 → 40 fps.
- Akustik filtre katsayısını önbelleğe almak (örnek başına `exp()` yerine).
- WebGL2, kopyasız uniform/buffer yüklemeleri, uniform önbelleği (çağrıların ~%92'si atlanıyor).
- Tam LTO (`lto = "fat"`, `codegen-units = 1`) ve daha hafif GL köprüsü: hedge3 kilitsiz ~34 → ~39 fps (headless, gürültülü ölçüm). Derleme ~11 dk.
- GL durum ve uniform tekrarlarını Rust tarafında atlamak (yamalı miniquad, [port/miniquad/LGWEB.md](../port/miniquad/LGWEB.md))
  ve `draw_batch`'te yalnızca görünen üçgenlerin köşelerini işlemek: kare başına uniform çağrısı 41.224 → 66,
  hedge3 kilitsiz ~39 → ~49 fps (headless, thin LTO, 3 tur).

## Bilinen eksikler

1. **Performans:** En ağır haritalarda (hedge2/hedge3) kilitsiz ~50 fps (headless). Kalan yük LG'nin
   çarpışma kodunda (kenar tutunma, NPC süpürmeleri), `render_fx`'te ve köşe verisinin her karede GPU'ya yüklenmesinde.
2. **LG kaynaklı sadakat farkları:** Ara sahne kurgusu, efektler ve düşman davranışı LG'nin kendi
   yaklaşımları (bkz. LG README "Current status"). Hız düzeltmesi sonrası LG'nin elle yazılmış bazı
   sahnelerinde zamanlama sorunu çıkabilir (`?fast-movement` geri dönüş yolu).
3. **Safari** test edilmedi. Firefox, kod düzeyinde desteklenen API'leri kullanıyor ama denenmedi.
4. **Tek sekme:** OPFS senkron tutamakları dosyaları kilitler.
5. **Upstream:** LG `055b1d1`. Yerel yamalar: `assets.rs` (wasm pk3 köprüsü), `audio.rs` (web
   çıkışı, park etme), `acoustics.rs`, `movement.rs` + `preferences.rs` (hızlar), `save.rs`,
   `viewer.rs` (yükleme karesi, WebGL2), `main.rs`, `input.rs` + kontrolcü ipuçları (`hud.rs`, `menu.rs`, `movie.rs`, `chapters.rs`), yeni `userfs.rs`, mekanik `as u64`,
   `render.rs` (`draw_batch` kısmi görünürlük). miniquad: `port/miniquad` (bkz. LGWEB.md).
