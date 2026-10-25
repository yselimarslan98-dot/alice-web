// Assembles the static site (GitHub Pages or any static host) into dist/:
// the page, worker, miniquad loader and the release wasm. No game data is included;
// players choose their own Alice folder in the browser.
// Usage: node tools/build-site.mjs [outDir=dist]   (after `npm run build:wasm`)
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const out = path.resolve(root, process.argv[2] || 'dist');
const wasm = path.join(root, 'port/lookingglass/target/wasm32-unknown-unknown/release/looking-glass.wasm');
if (!fs.existsSync(wasm)) {
  console.error('Missing wasm build: run `npm run build:wasm` first.');
  process.exit(1);
}
fs.rmSync(out, { recursive: true, force: true });
fs.cpSync(path.join(root, 'web'), out, { recursive: true });
fs.mkdirSync(path.join(out, 'wasm'), { recursive: true });
fs.copyFileSync(wasm, path.join(out, 'wasm/looking-glass.wasm'));
// Version every script and the wasm by content, so a cached page never pairs with another
// build's worker or engine (Pages caches files for 10 minutes).
const hash = crypto.createHash('sha256');
for (const f of ['main.js', 'worker.js', 's3.js', 'vendor/miniquad-gl.js', 'wasm/looking-glass.wasm']) {
  hash.update(fs.readFileSync(path.join(out, f)));
}
const v = hash.digest('hex').slice(0, 12);
const rewrite = (file, pairs) => {
  let text = fs.readFileSync(path.join(out, file), 'utf8');
  for (const [from, to] of pairs) {
    if (!text.includes(from)) throw new Error(`${file}: ${from} not found`);
    text = text.replaceAll(from, to);
  }
  fs.writeFileSync(path.join(out, file), text);
};
rewrite('index.html', [['<script src="main.js">', `<script src="main.js?v=${v}">`]]);
rewrite('main.js', [["new Worker('worker.js')", `new Worker('worker.js?v=${v}')`], ["import('./s3.js')", `import('./s3.js?v=${v}')`]]);
rewrite('worker.js', [["importScripts('vendor/miniquad-gl.js')", `importScripts('vendor/miniquad-gl.js?v=${v}')`],
  ["load('wasm/looking-glass.wasm')", `load('wasm/looking-glass.wasm?v=${v}')`]]);
for (const license of ['LICENSE-MIT', 'THIRD_PARTY_NOTICES.txt', 'LEGAL.md']) {
  fs.copyFileSync(path.join(root, 'port/lookingglass', license), path.join(out, `looking-glass-${license}`));
}
fs.writeFileSync(path.join(out, '.nojekyll'), '');
const size = (dir) => fs.readdirSync(dir, { withFileTypes: true })
  .reduce((a, e) => a + (e.isDirectory() ? size(path.join(dir, e.name)) : fs.statSync(path.join(dir, e.name)).size), 0);
console.log(`Site written to ${out} (${(size(out) / 1e6).toFixed(1)} MB, version ${v})`);

