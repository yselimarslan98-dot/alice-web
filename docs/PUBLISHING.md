# GitHub Pages'te yayınlama

Durum: yayında. Repo: `https://github.com/yselimarslan98-dot/alice-web`,
site: `https://yselimarslan98-dot.github.io/alice-web/`.

## Ne yayınlanıyor, ne yayınlanmıyor

| Yayınlanır | Yayınlanmaz |
| --- | --- |
| Sayfa (`web/`), Worker, miniquad yükleyicisi (MIT/Apache) | Hiçbir `.pk3`, doku, ses, harita, video |
| Looking Glass motoru, wasm olarak (MIT; lisans dosyaları sitede) | Orijinal `alice.exe` / DLL'ler |
| Bu depodaki araçlar ve belgeler | `config.json`, `userdata/`, `dist/` (gitignore'da) |

Oyun verisi EA'ya aittir. "Abandonware" yasal bir statü değildir ve dosyaların başka bir yerde
bulunması onları yeniden yayınlama hakkı vermez. Site bu yüzden "kendi dosyanı getir" modeliyle
çalışıyor: her oyuncu kendi kopyasını kendi tarayıcısında seçer.

## Nasıl güncellenir

`master`'a her push'ta `Deploy site` iş akışı (`.github/workflows/pages.yml`) wasm'i derler ve
siteyi yeniden yayınlar (~5-10 dk). Pages kaynağı: **Settings → Pages → Source: GitHub Actions**.

## Gizlilik

- Commit'ler GitHub'ın gizli adresiyle (`<id>+<kullanıcı>@users.noreply.github.com`, yerel
  `git config`) yazılıyor. Yayın öncesi geçmiş tek bir commit'e indirildi; eski geçmiş yalnızca
  yerelde `backup-history` dalında duruyor ve **push edilmemeli.**
- `docs/evidence/phase0-rea-bundle.json.gz` içindeki yerel yollar `<user>` ile değiştirildi.

## Bilinen sınırlamalar (sitede)

- Gerekli tarayıcı özellikleri: OPFS (senkron erişim tutamakları dahil), OffscreenCanvas + WebGL2,
  Web Worker. Güncel Chrome, Edge ve Firefox destekler. Safari test edilmedi.
- İlk kurulumda diskte ~700 MB yer gerekir (tarayıcının site deposu).
- Aynı anda yalnızca bir sekmede oynanabilir (OPFS senkron tutamakları dosyayı kilitler).
- `kullanıcı.github.io` altındaki tüm Pages projeleri aynı kökeni paylaşır. Veriler bu yüzden
  OPFS'te `alice-web/` klasöründe tutuluyor.
