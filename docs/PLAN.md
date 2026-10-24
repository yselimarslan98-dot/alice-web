# alice-web — Mimari karar (Aşama 0)

> Durum (2026-10-23): Kullanıcı önce **(c) fizibilite denemesini** seçti. Deneme **4/4 ölçütü
> geçti**, sonuçlar [SPIKE-C.md](SPIKE-C.md)'de. (c) ile devam kararı onay bekliyor.
> Dayanak: [FINDINGS.md](FINDINGS.md).

## Hedef ve sabit kurallar

- American McGee's Alice (2011 vanilla verisi) localhost'ta tarayıcıda oynanabilir olacak.
  Kullanım kişisel, hiçbir şey yayınlanmıyor.
- Orijinal dosyalar salt okunur. Varlıklar depoya kopyalanmaz; yerel sunucu `.pk3`'leri
  çalışma anında okur. Yol `config.json` içinde (`gameBase`).
- Her aşama tarayıcıda denenebilir bir sürümle biter ve commit'lenir.

## Her yolda ortak olan parça: yerel varlık sunucusu

Node 22 (kurulu) ile `npm run dev`:

1. `config.json` içindeki `gameBase` → `bin/base/*.pk3` ve `loc/*.pk3` merkezi dizinleri bir kez okunur
   (`tools/pk3-inventory.mjs` içindeki okuyucu ile; 9 arşiv yaklaşık 6 saniyede tarandı).
2. Birleşik sanal dosya sistemi. Öncelik: alfabetik pk3 sırası (sonraki kazanır), ardından seçilen dil
   paketi. `GET /vfs/<yol>` tek bir girdiyi bellekte açıp döndürür.
3. Gerekirse `GET /pk3/<ad>` ham arşivi HTTP **Range** desteğiyle sunar.
4. Sunucu yalnızca `127.0.0.1` adresini dinler.

## Seçenekler

### (a) Açık kaynak C/C++ motorunu uyarlamak + Emscripten

Taban olarak OpenMoHAA (GPL-2) veya saf ioquake3 (GPL-2, resmi Emscripten desteği var).

| | |
| --- | --- |
| Motor tarafı | BSP v42 (MOHAA'nın lump düzeni farklı), FTX, TAN v2, SKB/SKA v2–3 ve Alice shader anahtar kelimeleri uyarlanacak. Q3 FS pk3'ü doğrudan okuyabiliyor, ama 900 MB'lık arşivler wasm belleğine sığmaz: Range tabanlı özel bir FS katmanı gerekir |
| Oyun tarafı | OpenMoHAA'nın fgame'i **MOHAA oyunu**, Alice'e uymuyor. Alice fgame'i (Morpheus benzeri derleyici + VM, 252 entity sınıfı, `.st` AI'ları, silahlar) C++ ile sıfırdan yazılmalı. FAKK2 SDK en yakın ata, ama Ritual EULA'sı altında; kodunu kopyalamak, hele GPL kodla karıştırmak lisans riski taşır |
| Araçlar | emsdk, CMake ve Windows'ta C++ derleme zinciri kurulmalı. Tarayıcıda C++ hata ayıklamak yavaş |
| İlk oynanabilir hedef (haritada yürüme + çarpışma) | ~2–3 ay |
| Tam kampanya | ~9–18 ay |
| Riskler | Büyük yabancı kod tabanı. Motor kazancı, oyun mantığının sıfırdan yazılma maliyeti yanında küçük kalıyor. Lisans karışıklığı |

### (b) Sıfırdan TypeScript + WebGL2 (three.js) motoru

| | |
| --- | --- |
| Motor tarafı | BSP v42 ayrıştırıcı (ilk 15 lump Q3 ile aynı, FINDINGS'te doğrulandı), patch tesselation, lightmap, PVS, Q3 shader → GLSL, TIKI/SKB/SKA/TAN yükleyicileri, BSP brush çarpışması (Q3 `CM_Trace` mantığı) |
| Oyun tarafı | `.scr` için TS'de derleyici + VM (dil küçük ve C benzeri: tipler, `wait`/`waitfor`/`thread` eşyordamları; JS generator'larıyla doğal biçimde yazılır). `.st` yorumlayıcısı. Entity sınıfları, en sık 20–30 komutla başlayıp kademeli genişler. **Orijinal script'leri çalıştırdığımız için** seviye başına elle kod yazmak gerekmez, kapsam doğal olarak genişler |
| Araçlar | Yalnız Node (kurulu) + Vite. Hata ayıklama tarayıcı DevTools'ta. A5'teki davranış analizi için Ghidra + JDK 21 gerekecek |
| Referanslar | Looking Glass `docs/` ve kodu (MIT; atıfla port edilebilir), FAKK2 SDK (yalnız davranış okuma, kopyalama yok) |
| İlk oynanabilir hedef | A1 (harita görüntüleme) **1–2 hafta**, A2 (yürüme/zıplama/çarpışma) +2–3 hafta, yani ~1–1,5 ay |
| Tam kampanya | ~9–15 ay (A5 ve A6 en büyük kalemler: AI, silahlar, boss'lar, ara sahneler) |
| Riskler | Uzun kuyruk: 1.607 farklı script komutu ve 252 sınıfın anlamı fgame'den çıkarılmalı. Q3 shader'larını birebir taklit etmek. Performans (harita başına 5–10 MB BSP ve yüzlerce doku). Bunların hepsi kademeli ve ölçülebilir riskler |

### (c) Ek seçenek: Looking Glass'ı (Rust, MIT) WebAssembly'ye taşımak

Araştırma sırasında çıktı, sonucu değiştirdiği için ekledim. Looking Glass aynı 2011 verisini
pk3'ten okuyor ve README'sine göre tüm kampanya oynanabilir durumda.

| | |
| --- | --- |
| Neden umut verici | macroquad'ın resmi `wasm32-unknown-unknown` hedefi var, shader'lar zaten GLSL `#version 100` (WebGL1). Oyun mantığının ~208 bin satırı hazır |
| Engeller (koddan sayıldı) | 326 noktada **senkron** pk3 okuma (web'de dosyalar asenkron gelir; ön yükleme veya senkron XHR gerekir), 100 dosyada `std::fs` (çoğu test ve doğrulama araçları), 2 thread, 13 `Instant`/`SystemTime` kullanımı (wasm32'de panik verir), rodio ses katmanı (web'de yeniden yazılmalı), kayıtlar diske yazılıyor |
| Araçlar | **Rust kurulumu gerekiyor** (rustup + `wasm32-unknown-unknown`). Şu an makinede yok |
| İlk oynanabilir hedef | Engeller beklendiği kadar ise ~3–6 hafta, ve doğrudan **tüm kampanya** |
| Riskler | Kendimizin olmayan, hızlı değişen (son commit 2 gün önce), yapay zekâ destekli deneysel bir kod tabanının fork'unu sürdürmek. Bellek (yalnız pak0+pak1 ≈ 450 MB). Ses. Sadakat Looking Glass'ın elle yazılmış yaklaşımlarıyla sınırlı (orijinal script'leri bilerek çalıştırmıyor). Senin A1–A6 aşama yapın bu yola uymaz |

## Karşılaştırma

| Ölçüt | (a) C++/Emscripten | (b) TypeScript | (c) LG → wasm |
| --- | --- | --- | --- |
| İlk tarayıcı görüntüsü | 1–2 ay | **1–2 hafta** | 1–3 hafta (belirsiz) |
| Tam kampanya | 9–18 ay | 9–15 ay | **~1–2 ay** (başarırsa) |
| Kod sahipliği ve anlaşılırlık | düşük | **yüksek** | orta |
| Orijinal script'lere sadakat | yüksek (VM yazılırsa) | **yüksek** (VM) | düşük–orta (elle yazılmış) |
| Kurulum ihtiyacı | emsdk, CMake | **yok** | Rust |
| Lisans riski | orta (GPL + EULA) | **düşük** | düşük (MIT) |
| A1–A6 planına uyum | orta | **tam** | zayıf |

## Tavsiye

**(a) önerilmez.** C++ motoru motor tarafında az şey kazandırıyor. Oyun mantığı zaten sıfırdan
yazılacak, üstüne derleme zinciri ve lisans karmaşası ekleniyor.

**Ana tavsiyem (b): TypeScript + three.js, senin A1–A6 planınla.** Gerekçeler:

1. İlk görüntü en hızlı bu yolda (A1 1–2 hafta) ve her aşama doğal olarak tarayıcıda test edilebilir.
2. `.scr`/`.st` için gerçek bir yorumlayıcı yazmak, orijinal 864 KB script'i **olduğu gibi**
   çalıştırır. Bu da 39 ziyaret için tek tek elle kod yazmaktan hem daha sadık hem daha ölçeklenebilir.
3. Looking Glass'ın MIT belgeleri ve kodu, format ayrıntıları ve sabitler için (atıfla)
   kullanılarak (b)'nin süresi kısaltılabilir.

**Alternatif karar noktası (c):** Asıl isteğin "tam oyunu en kısa sürede tarayıcıda oynamak"
ise, (c) aylar kazandırabilir. Bu durumda önce **en fazla 5 iş günlük bir fizibilite denemesi**
öneririm. Başarı ölçütleri:

1. Rust kurulur ve LG yerelde derlenip açılır.
2. `wasm32` derlemesi Chrome'da ana menüyü gösterir.
3. İlk seviye, orijinal pk3'lerden sunucu üzerinden (çıkarmadan) yüklenip ≥30 fps çalışır.
4. Sekme belleği 2 GB'ın altında kalır.

Ölçütler tutmazsa (b)'ye geçilir. Fizibilite süresi kayıp olmaz: sunucu ve VFS iki yolda da ortak.

## Onaydan sonraki ilk adım (b seçilirse — A1)

- `package.json` (Vite + TypeScript + three.js), `config.example.json` → `config.json`.
- `server/`: pk3 VFS + `/vfs/*` uç noktası (Range, önbellek başlıkları).
- `src/formats/bsp.ts` (FAKK v42), `ftx.ts`, `tga.ts`, `shader.ts` (önce `map`/lightmap ve basit blend).
- Harita seçici + serbest uçuş kamerası. Test: `npm run dev` → `http://127.0.0.1:5173/?map=hedge1`.

## Senden kararlar

1. **(b)** ile mi devam edelim, yoksa önce **(c)** fizibilite denemesi mi yapalım?
2. (c) seçilirse: Rust araç zinciri (rustup, kullanıcı dizinine) kurulabilir mi?
3. A5 öncesi Ghidra 12.x + JDK 21 (taşınabilir, kullanıcı dizinine) kurulabilir mi?
   REA'nın decompile araçları ancak bununla çalışır.
4. Dil: varsayılan altyazı `pak4_english.pk3` içindeki **Türkçe** metin, ses ise İngilizce.
   Bu kalsın mı?
