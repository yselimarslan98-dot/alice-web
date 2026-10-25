# Port durumu (Looking Glass → tarayıcı)

Son güncelleme: 2026-10-25. Başlangıç denemesi: [SPIKE-C.md](SPIKE-C.md). Yayınlama: [PUBLISHING.md](PUBLISHING.md).

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
| "Safe footing" (son güvenli zemine dön) klavyede R yerine Backspace (yanlışlıkla basılmasın; eski ayarlar bir kez taşınır, kontrolcüde D-pad sol aynen) | ✅ |

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
   `render.rs` (`draw_batch` kısmi görünürlük), düşman yapay zekası (aşağıda). miniquad: `port/miniquad` (bkz. LGWEB.md).
6. **Hareket testleri:** Orijinal hızlara geçişten beri `movement::tests` içindeki 4 test eski 320 u/s
   koşma hızını varsayıyor ve başarısız oluyor (oyun davranışı doğru; testler güncellenmedi).

## Düşman yapay zekası (yerel geliştirme)

LG'nin düşman kodu upstream ile birebir aynıydı; "aptal" davranış port hatası değil, LG'nin
basitleştirilmiş yaklaşımı. Orijinal durum makineleri oyun verisinde duruyor (`pak0.pk3` → `ai/*.st`,
58 dosya); LG bunları okumuyor. Yerel iyileştirmeler bu dosyalardaki değerlere göre (yeni `combat/alert.rs`):

- **Takip (`HAVE_ENEMY` + `GetCloseToEnemy`):** Alice'i gören düşman 15 sn boyunca (`alert::LEASH`, her
  görüşte yenilenir) görmese de onun gerçek konumuna yürüyor; kart askeri, satranç, saat askeri, ateş imp'i,
  karınca, karınca aslanı.
- **Yol bulma:** Kart askerleri, satranç taşları, saat askerleri ve ateş imp'leri karıncaların kullandığı
  `combat::navigation::Detour` ile engellerin etrafından dolaşıyor (eskiden düz çizgi + 5 açı).
- **Haberleşme (`NOISE_HEARD`):** Alice'i ilk gören ya da boştayken vurulan düşman bir uyarı yayıyor; 360 birim
  içinde veya 900 birime kadar açık görüşteki boştaki düşmanlar uyanıyor (gizli karınca aslanları da çıkıyor).
  Uyarılar 1,5 sn yaşar, kaydedilmez, harita değişince temizlenir.
- **Saldırı sıklığı (`ANIM_DONE`):** LG her 0,7 sn'de zar atıyordu; artık yürüme/koşma döngüsü başına bir kez,
  orijinal olasılık ve menzillerle (`cards::Kind::tactics`): maça yürürken %29,2 (>150), koşarken %72,1;
  kupa yürürken %40, koşarken en az 2 sn sonra %30, yakın dövüş 100/120; boştayken ilk atış maça %40, kupa %66,6;
  yol yoksa (`HOLD_POSITION`) durup %75/%50. Saat askeri: döngü başına %60 (walk_slow) / %33 (walk_norm).
  Karınca çavuşu: %30 ve 150–640 birim.
- **Hızlı yürüyüş:** Satranç taşları uzaktayken `walk_2` (at/fil 400, kale 500), karıncalar görünür ve 250'den
  uzaktayken `walk_fast` (230 altında döngü sonunda bırakır).
- **Sinek / karo askerleri (`combat.rs` `Guard`, ayrı bir uygulama; fortress, skool, gvillage, pandemonium):**
  eskiden 420/900 birim görüş, başlangıç noktasından 500–700 birim bağ, sıfır hafıza, sabit 95 u/s düz yürüyüş ve
  karo için 600 birim içinde her 0,65 sn'de atış vardı. Artık 1000/1200 görüş, takip + yol bulma, klip hızlarıyla
  yürüme/koşma (`Gait`, yeni `State::Run`; sinek 150, karo 250 ötesinde koşar), yakın dövüş 100 birim; karo
  boştayken %50 hemen atış, yürürken döngü başına %33,3, koşarken %20, yol yoksa %80 (>250).
- **Kötü mantar çekişi (`plants.rs`):** LG her 1/120 sn adımda Alice'e geri itme (knockback) veriyordu; bu
  hem 1200 u/s² sabit hızlanma demekti hem de girdi ve sürtünmeyi sürekli kapatıyordu, Alice çaresizce
  hızlanarak çekiliyordu. Orijinal `SuckEnemy::Evaluate` (`fgamex86.dll`, 0x1003e820): hıza
  `(448 − mesafe) / 384 × 1200` u/s² ekler; oyuncu sürtünmesi bunu 600 u/s² altında sıfırlar, üstünde
  ivme/6 hızında bir kaymaya indirir. Artık ~256 birimden uzakta çekiş yok, yakında 100–233 u/s kayma
  (`Feedback::drift`, `Player::drift`); Alice kontrolü kaybetmiyor.
- Ateş imp'i, karınca aslanı, boojum, fil ışını ve kale hücumu zaten orijinale yakındı; yalnızca kayıt eklendi.
- Kayıt uyumu: yeni alanlar `#[serde(default)]`; eski kayıtlar yüklenir.
- **Yapay zeka kaydı (`?ai-log`, yerelde `--ai-log`):** `ai_log.rs` her düşmanı durum değişiminde ve
  takipteyken 0,5 sn'de bir, Alice'i 0,5 sn'de bir kaydeder; 10 sn'de bir kullanıcı verisine
  `ai-log.txt` olarak yazar (en fazla ~4 MB, kayıt dışa aktarımına girmez). Ayarlar → "Yapay zeka kaydı → İndir".
