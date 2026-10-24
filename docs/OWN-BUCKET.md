# Oyun dosyalarını kendi özel depondan yüklemek

Amaç: Yeni bir cihazda klasör seçmeden oynamak. Oyun dosyaları yalnızca senin erişebildiğin özel bir
bulut kovasında durur. Siteyi kişisel kurulum bağlantınla açınca dosyalar oradan o tarayıcıya
**bir kez** iner, sonraki girişlerde oyun doğrudan açılır.

- Kova **özel** kalır. Herkese açık erişim açılmaz; site her isteği imzalar (AWS Signature V4, `web/s3.js`).
- Bağlantının içinde **yalnızca okuma** yetkili bir anahtar vardır. Anahtar adresin `#` sonrasındaki
  kısmında taşınır, bu kısım hiçbir sunucuya (GitHub dahil) gönderilmez. Site anahtarı açılışta adres
  çubuğundan siler ve yalnızca o tarayıcının site deposunda (`remote.json`) saklar.
- Bağlantıyı başkasıyla paylaşma: bağlantıya sahip olan herkes dosyaları indirebilir.
  Bağlantı sızarsa okuma anahtarını panelden silip yenisini oluştur.

## 1. Kova oluşturma (Cloudflare R2 örneği)

1. Cloudflare hesabı aç, **R2 Object Storage**'ı etkinleştir. Ücretsiz katman 10 GB'tır ve indirme
   ücreti yoktur. Etkinleştirme için ödeme yöntemi istenebilir.
2. **Create bucket** → ad: `alice`.
3. **Manage R2 API Tokens** bölümünden iki anahtar oluştur. Her birinden *Access Key ID* ve
   *Secret Access Key* değerlerini al:
   - Yükleme için: **Admin Read & Write**. Bu yetki CORS ayarını da yapabilir. Yükleme bitince
     silebilirsin.
   - Bağlantı için: **Object Read only**, yalnızca `alice` kovası.
4. Hesap kimliğin (Account ID) R2 sayfasının sağında yazar. Uç nokta:
   `https://<ACCOUNT_ID>.r2.cloudflarestorage.com`.

Backblaze B2, AWS S3 ya da başka bir S3 uyumlu servis de çalışır. Bu durumda `endpoint` ve `region`
değerlerini o servisin değerleriyle değiştir.

## 2. Yükleme (bu bilgisayardan, bir kez)

```sh
cp s3.example.json s3.json     # anahtarları doldur; s3.json gitignore'da, asla commit'lenmez
npm run upload:bucket
```

Araç sırasıyla şunları yapar:
- Kovanın CORS kuralını siteye izin verecek şekilde ayarlar. Yapamazsa panele girilecek kuralı yazdırır.
- `config.json` → `gameBase` içindeki 6 pk3'ü yükler. Orijinal dosyalar yalnızca okunur. Kovada aynı
  boyutta olan dosya atlanır.
- Okuma anahtarıyla kovayı listeleyerek bağlantıyı doğrular.
- **Kişisel kurulum bağlantısını** yazdırır.

## 3. Kısa şifre (isteğe bağlı)

```sh
npm run seal:bucket            # rastgele 12 karakterlik şifre üretir; ya da: npm run seal:bucket -- <şifre>
```

Okuma anahtarı bu şifreyle şifrelenir (PBKDF2-SHA256 600.000 tur + AES-256-GCM) ve `web/bucket.sealed.json`
olarak siteye konur. Şifreyi bilmeyen biri anahtarı çözemez. Yeni bir bilgisayarda kurulum ekranına
şifreyi yazmak ya da `…/alice-web/#k=<şifre>` adresini açmak yeterlidir. Şifreyi değiştirmek için komutu
yeniden çalıştırıp push et; eski şifre artık çalışmaz.

## 4. Oynama

Bağlantıyı yer imlerine ekle. Herhangi bir cihazda aç; dosyalar iner (~660 MB) ve oyun başlar. Ayrıca
kurulum ekranındaki kutuya da yapıştırılabilir.

## Test (bulut hesabı olmadan)

```sh
npm run mock:bucket      # 127.0.0.1:9000: imza doğrulayan, CORS'lu sahte kova (gameBase'den okur)
npm run build:site && npm run serve:site
# mock:bucket'ın yazdırdığı bağlantıyı aç
npm run test:s3          # imzalayıcıyı AWS'nin yayınladığı örneklerle doğrular
```
