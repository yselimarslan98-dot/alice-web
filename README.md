# alice-web

American McGee's Alice (2011) tarayıcıda. Motor, [Looking Glass](https://github.com/looking-glass-project/LookingGlass)
Rust motorunun (MIT) WebAssembly derlemesi (`port/lookingglass/`). Oyun bir Web Worker'da,
OffscreenCanvas + WebGL2 ile çalışır.

**Bu depo ve yayınlanan site hiçbir oyun dosyası içermez.** Oyuncu kendi Alice kurulumunu seçer.
Dosyalar yalnızca o tarayıcının bu siteye ayrılmış özel deposuna (OPFS) kopyalanır, hiçbir
sunucuya yüklenmez. Sonraki ziyaretlerde oyun doğrudan açılır. Kayıtlar da tarayıcıda saklanır.

- [docs/PUBLISHING.md](docs/PUBLISHING.md): GitHub Pages'te yayınlama adımları
- [docs/PORT-STATUS.md](docs/PORT-STATUS.md): güncel durum, ölçümler, bilinen eksikler
- [docs/FINDINGS.md](docs/FINDINGS.md) · [docs/PLAN.md](docs/PLAN.md) · [docs/SPIKE-C.md](docs/SPIKE-C.md): keşif ve kararlar

## Yerelde oynamak (kopyalamadan)

Gerekenler: Node 22+, Rust (`rustup target add wasm32-unknown-unknown`), Chrome / Edge / Firefox.

```sh
cp config.example.json config.json   # gameBase: Alice1/bin/base klasörünün yolu
npm run build:wasm                   # port/lookingglass -> wasm32 (~3-6 dk)
npm run dev                          # http://127.0.0.1:8080/
```

Geliştirme sunucusu orijinal `.pk3` dosyalarını yerinde, salt okunur olarak sunar (HTTP range).
Tarayıcıya kopyalama yapılmaz.

## Statik site (GitHub Pages ile aynı)

```sh
npm run build:wasm && npm run build:site   # dist/ (~24 MB, oyun verisi yok)
npm run serve:site                         # http://127.0.0.1:8090/
```

İlk açılışta "Alice klasörünü seç" ekranı gelir. `Alice1` ya da `Alice1\bin` ya da
`Alice1\bin\base` seçilebilir; yalnızca `bin/base/*.pk3` (~660 MB) kopyalanır.

## Kontroller ve seçenekler

- Oyun alanına tıkla: fare kilitlenir (Esc bırakır). WASD + fare, Space zıplama, Shift yürüme, F5/F9 hızlı kayıt/yükleme, Tab bölümler.
- `?map=hedge1`, `?load=quick`, `?fast-movement` gibi sorgu parametreleri Looking Glass seçeneklerine çevrilir.
- `#fps=30` / `#fps=0` (sınırsız) kare sınırını değiştirir (varsayılan 60); `#nohud` tanı satırını gizler.
- Sağ üstteki "Bu sitedeki verileri sil": kopyalanan oyun dosyalarını ve kayıtları siler.
