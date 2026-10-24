// Local stand-in for the player's bucket, for testing the setup-link flow without a cloud
// account: a path-style S3 endpoint that checks SigV4 signatures, answers CORS preflights and
// serves config.json gameBase/*.pk3 (read-only) as objects of one bucket.
// Usage: node tools/mock-s3.mjs [port=9000]  -> prints a setup link for http://127.0.0.1:8090/
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { encodeConfig, signRequest } from '../web/s3.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const gameBase = path.resolve(root, JSON.parse(fs.readFileSync(path.join(root, 'config.json'), 'utf8')).gameBase);
const port = Number(process.argv[2] || 9000);
const cfg = { endpoint: `http://127.0.0.1:${port}`, bucket: 'alice', region: 'auto', accessKeyId: 'TESTKEY', secretAccessKey: 'testsecret' };
const packs = fs.readdirSync(gameBase).filter((f) => /\.pk3$/i.test(f));
const cors = { 'Access-Control-Allow-Origin': '*', 'Access-Control-Allow-Methods': 'GET, HEAD',
  'Access-Control-Allow-Headers': '*', 'Access-Control-Expose-Headers': 'Content-Length' };

async function signatureOk(req) {
  const auth = /SignedHeaders=([^,]+), Signature=(\w+)/.exec(req.headers.authorization || '');
  const amz = req.headers['x-amz-date'] || '';
  if (!auth || !amz) return false;
  const headers = Object.fromEntries(auth[1].split(';').filter((h) => h !== 'host' && !h.startsWith('x-amz-'))
    .map((h) => [h, req.headers[h]]));
  const date = new Date(amz.replace(/(\d{4})(\d\d)(\d\d)T(\d\d)(\d\d)(\d\d)Z/, '$1-$2-$3T$4:$5:$6Z'));
  const expected = await signRequest({ method: req.method, url: `http://${req.headers.host}${req.url}`, headers,
    payloadHash: req.headers['x-amz-content-sha256'], region: cfg.region, accessKeyId: cfg.accessKeyId,
    secretAccessKey: cfg.secretAccessKey, date });
  return expected.authorization.endsWith(`Signature=${auth[2]}`);
}

http.createServer(async (req, res) => {
  if (req.method === 'OPTIONS') { res.writeHead(204, cors); return res.end(); }
  if (!(await signatureOk(req))) {
    res.writeHead(403, { ...cors, 'Content-Type': 'application/xml' });
    return res.end('<Error><Code>SignatureDoesNotMatch</Code></Error>');
  }
  const url = new URL(req.url, 'http://x');
  if (url.pathname === '/alice' && url.searchParams.get('list-type') === '2') {
    const items = packs.map((n) => `<Contents><Key>${n}</Key><Size>${fs.statSync(path.join(gameBase, n)).size}</Size></Contents>`);
    res.writeHead(200, { ...cors, 'Content-Type': 'application/xml' });
    return res.end(`<ListBucketResult><IsTruncated>false</IsTruncated>${items.join('')}</ListBucketResult>`);
  }
  const name = decodeURIComponent(url.pathname.replace(/^\/alice\//, ''));
  if (packs.includes(name)) {
    const file = path.join(gameBase, name);
    res.writeHead(200, { ...cors, 'Content-Type': 'application/octet-stream', 'Content-Length': fs.statSync(file).size });
    return fs.createReadStream(file).pipe(res);
  }
  res.writeHead(404, cors);
  res.end('<Error><Code>NoSuchKey</Code></Error>');
}).listen(port, '127.0.0.1', () => {
  console.log(`mock S3: ${cfg.endpoint}/alice (${packs.length} packs)`);
  console.log(`setup link: http://127.0.0.1:8090/#remote=${encodeConfig(cfg)}`);
});
