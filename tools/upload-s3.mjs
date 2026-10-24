// Copies the player's own game PK3s (config.json gameBase, read-only) into their private
// S3-compatible bucket, sets the bucket's CORS rule for the site, and prints the personal
// setup link. Keys come from s3.json (gitignored, never published):
//   { "endpoint": "https://<account>.r2.cloudflarestorage.com", "bucket": "alice", "region": "auto",
//     "prefix": "", "accessKeyId": "<read+write key>", "secretAccessKey": "...",
//     "readAccessKeyId": "<read-only key for the link>", "readSecretAccessKey": "...",
//     "origins": ["https://<user>.github.io"], "siteUrl": "https://<user>.github.io/alice-web/" }
// Usage: node tools/upload-s3.mjs [--cors-only] [--link-only]
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import https from 'node:https';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { encodeConfig, listObjects, objectUrl, s3fetch, signRequest } from '../web/s3.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (f) => JSON.parse(fs.readFileSync(path.join(root, f), 'utf8'));
const s3 = read('s3.json');
const gameBase = path.resolve(root, read('config.json').gameBase);
const args = new Set(process.argv.slice(2));
const conn = { endpoint: s3.endpoint, bucket: s3.bucket, region: s3.region || 'auto', prefix: s3.prefix || '' };
const writer = { ...conn, accessKeyId: s3.accessKeyId, secretAccessKey: s3.secretAccessKey };

async function putCors() {
  const origins = s3.origins || [new URL(s3.siteUrl).origin];
  const xml = '<CORSConfiguration><CORSRule>' + origins.map((o) => `<AllowedOrigin>${o}</AllowedOrigin>`).join('') +
    '<AllowedMethod>GET</AllowedMethod><AllowedMethod>HEAD</AllowedMethod><AllowedHeader>*</AllowedHeader>' +
    '<ExposeHeader>Content-Length</ExposeHeader><MaxAgeSeconds>3600</MaxAgeSeconds></CORSRule></CORSConfiguration>';
  const body = Buffer.from(xml);
  const url = `${s3.endpoint.replace(/\/+$/, '')}/${encodeURIComponent(s3.bucket)}?cors`;
  await s3fetch(writer, 'PUT', url, {
    body,
    headers: { 'content-md5': createHash('md5').update(body).digest('base64'), 'content-type': 'application/xml' },
    payloadHash: createHash('sha256').update(body).digest('hex'),
  });
  console.log(`CORS: ${origins.join(', ')}`);
}

// Streams one file with a PUT (node:https, so a 290 MB file never sits in memory).
async function upload(file, name) {
  const size = fs.statSync(file).size;
  const url = objectUrl(writer, name);
  const headers = await signRequest({ method: 'PUT', url, headers: { 'content-length': size },
    region: writer.region, accessKeyId: writer.accessKeyId, secretAccessKey: writer.secretAccessKey });
  await new Promise((resolve, reject) => {
    const req = https.request(url, { method: 'PUT', headers }, (res) => {
      let text = '';
      res.on('data', (d) => { text += d; });
      res.on('end', () => (res.statusCode < 300 ? resolve() : reject(new Error(`PUT ${name}: HTTP ${res.statusCode} ${text.slice(0, 300)}`))));
    });
    req.on('error', reject);
    let sent = 0, last = 0;
    fs.createReadStream(file).on('data', (d) => {
      sent += d.length;
      if (sent - last > 20e6 || sent === size) { last = sent; process.stdout.write(`\r  ${name}: ${(sent / 1e6).toFixed(0)} / ${(size / 1e6).toFixed(0)} MB`); }
    }).pipe(req);
  });
  process.stdout.write('\n');
}

if (!args.has('--link-only')) {
  // An object-only key may not change bucket settings; the rule can also be set in the dashboard.
  await putCors().catch((err) => {
    const origins = s3.origins || [new URL(s3.siteUrl).origin];
    console.warn(`CORS ayarlanamadı (${err.message}). Depo panelinde CORS kuralı olarak şunu girin:
` +
      JSON.stringify([{ AllowedOrigins: origins, AllowedMethods: ['GET', 'HEAD'], AllowedHeaders: ['*'], ExposeHeaders: ['Content-Length'], MaxAgeSeconds: 3600 }], null, 2));
  });
  if (!args.has('--cors-only')) {
    const remote = new Map((await listObjects(writer)).map((o) => [o.name.toLowerCase(), o.size]));
    for (const name of fs.readdirSync(gameBase).filter((f) => /\.pk3$/i.test(f))) {
      const file = path.join(gameBase, name);
      if (remote.get(name.toLowerCase()) === fs.statSync(file).size) { console.log(`  ${name}: zaten var`); continue; }
      await upload(file, name);
    }
  }
}

const link = { ...conn, accessKeyId: s3.readAccessKeyId || s3.accessKeyId, secretAccessKey: s3.readSecretAccessKey || s3.secretAccessKey };
const check = await listObjects(link);
console.log(`Okuma anahtarıyla görülen dosyalar: ${check.map((o) => o.name).join(', ')}`);
console.log(`\nKişisel kurulum bağlantısı (gizli tut, içinde okuma anahtarı var):\n${s3.siteUrl.replace(/#.*/, '')}#remote=${encodeConfig(link)}`);
