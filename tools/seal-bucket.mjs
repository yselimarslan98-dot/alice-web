// Publishes the read-only bucket config from s3.json encrypted under a short access code
// (web/bucket.sealed.json), so a new computer needs only the code instead of the long link.
// Usage: node tools/seal-bucket.mjs [code]   (without a code, a random one is generated)
import { randomInt } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { listObjects, normalizeCode, sealConfig, unsealConfig } from '../web/s3.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const s3 = JSON.parse(fs.readFileSync(path.join(root, 's3.json'), 'utf8'));
const config = { endpoint: s3.endpoint, bucket: s3.bucket, region: s3.region || 'auto', prefix: s3.prefix || '',
  accessKeyId: s3.readAccessKeyId, secretAccessKey: s3.readSecretAccessKey };

// 12 characters from an alphabet without look-alikes (0/o, 1/l/i): ~59 bits.
const alphabet = '23456789abcdefghjkmnpqrstuvwxyz';
const code = process.argv[2] || Array.from({ length: 3 }, () =>
  Array.from({ length: 4 }, () => alphabet[randomInt(alphabet.length)]).join('')).join('-');
if (normalizeCode(code).length < 12) throw new Error('Kod en az 12 harf/rakam olmalı.');

await listObjects(config); // the read-only key must work before it is published
const sealed = await sealConfig(config, code);
await unsealConfig(sealed, code);
fs.writeFileSync(path.join(root, 'web/bucket.sealed.json'), JSON.stringify(sealed) + '\n');
console.log(`web/bucket.sealed.json yazıldı.\nŞifre: ${code}\nKısa bağlantı: ${s3.siteUrl.replace(/#.*/, '')}#k=${code}`);
