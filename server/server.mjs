// Local-only dev server. Serves the page and lets the game worker read the original .pk3
// archives in place (read-only, HTTP range requests), so local play needs no copy into the
// browser. A static host (GitHub Pages) serves the same page without /pk3; the page then
// asks the player for their own Alice folder instead.
//   GET /pk3/list.json   -> [{ name, size }] for gameBase/*.pk3 (what Looking Glass mounts)
//   GET /pk3/<name>      -> archive bytes, honouring Range
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const configPath = path.join(root, 'config.json');
if (!fs.existsSync(configPath)) {
  console.error('config.json missing: copy config.example.json to config.json and set gameBase.');
  process.exit(1);
}
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
const gameBase = path.resolve(root, config.gameBase);
const port = Number(process.env.PORT || config.port || 8080);

const packs = fs.readdirSync(gameBase).filter((f) => /\.pk3$/i.test(f))
  .map((name) => ({ name, size: fs.statSync(path.join(gameBase, name)).size }));
console.log(`Serving ${packs.length} packs from ${gameBase}`);

const staticRoots = {
  '/': path.join(root, 'web'),
  '/wasm/': path.join(root, 'port/lookingglass/target/wasm32-unknown-unknown/release'),
};
const types = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css' };

function sendPack(req, res, name) {
  const pack = packs.find((p) => p.name.toLowerCase() === name.toLowerCase());
  if (!pack) { res.writeHead(404); return res.end(); }
  const file = path.join(gameBase, pack.name);
  const range = /^bytes=(\d+)-(\d+)?$/.exec(req.headers.range || '');
  if (!range) {
    res.writeHead(200, { 'Content-Type': 'application/octet-stream', 'Content-Length': pack.size, 'Accept-Ranges': 'bytes' });
    return fs.createReadStream(file).pipe(res);
  }
  const start = Number(range[1]);
  const end = Math.min(range[2] === undefined ? pack.size - 1 : Number(range[2]), pack.size - 1);
  if (start > end) { res.writeHead(416, { 'Content-Range': `bytes */${pack.size}` }); return res.end(); }
  const length = end - start + 1;
  const buffer = Buffer.alloc(length);
  const fd = fs.openSync(file, 'r');
  try { fs.readSync(fd, buffer, 0, length, start); } finally { fs.closeSync(fd); }
  res.writeHead(206, {
    'Content-Type': 'application/octet-stream',
    'Content-Length': length,
    'Content-Range': `bytes ${start}-${end}/${pack.size}`,
    'Cache-Control': 'no-cache',
  });
  res.end(buffer);
}

function sendFile(res, file) {
  if (!fs.existsSync(file) || !fs.statSync(file).isFile()) return false;
  res.writeHead(200, { 'Content-Type': types[path.extname(file)] || 'application/octet-stream', 'Cache-Control': 'no-cache' });
  fs.createReadStream(file).pipe(res);
  return true;
}

// Saves written by the earlier server-backed build (userdata/); the page imports them into
// browser storage once, so existing progress carries over.
const legacyUserData = path.join(root, 'userdata');
const listFiles = (dir, prefix = '') => !fs.existsSync(dir) ? [] : fs.readdirSync(dir, { withFileTypes: true })
  .flatMap((e) => e.isDirectory() ? listFiles(path.join(dir, e.name), `${prefix}${e.name}/`) : [`${prefix}${e.name}`]);

const server = http.createServer((req, res) => {
  const url = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  try {
    if (url === '/legacy-userdata.json') {
      res.writeHead(200, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' });
      return res.end(JSON.stringify(listFiles(legacyUserData)));
    }
    if (url.startsWith('/legacy-userdata/')) {
      const file = path.resolve(legacyUserData, url.slice('/legacy-userdata/'.length));
      if (file.startsWith(legacyUserData + path.sep) && sendFile(res, file)) return;
      res.writeHead(404);
      return res.end();
    }
    if (url === '/pk3/list.json') {
      res.writeHead(200, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' });
      return res.end(JSON.stringify(packs));
    }
    if (url.startsWith('/pk3/')) return sendPack(req, res, url.slice('/pk3/'.length));
    for (const [prefix, dir] of Object.entries(staticRoots).sort((a, b) => b[0].length - a[0].length)) {
      if (!url.startsWith(prefix)) continue;
      const file = path.resolve(dir, url.slice(prefix.length) || 'index.html');
      if (file.startsWith(dir) && sendFile(res, file)) return;
    }
    res.writeHead(404);
    res.end('not found');
  } catch (err) {
    console.error(url, err);
    res.writeHead(500);
    res.end(String(err));
  }
});
server.listen(port, '127.0.0.1', () => console.log(`alice-web: http://127.0.0.1:${port}/`));
