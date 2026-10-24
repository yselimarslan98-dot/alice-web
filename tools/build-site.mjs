// Assembles the static site (GitHub Pages or any static host) into dist/:
// the page, worker, miniquad loader and the release wasm. No game data is included;
// players choose their own Alice folder in the browser.
// Usage: node tools/build-site.mjs [outDir=dist]   (after `npm run build:wasm`)
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
for (const license of ['LICENSE-MIT', 'THIRD_PARTY_NOTICES.txt', 'LEGAL.md']) {
  fs.copyFileSync(path.join(root, 'port/lookingglass', license), path.join(out, `looking-glass-${license}`));
}
fs.writeFileSync(path.join(out, '.nojekyll'), '');
const size = (dir) => fs.readdirSync(dir, { withFileTypes: true })
  .reduce((a, e) => a + (e.isDirectory() ? size(path.join(dir, e.name)) : fs.statSync(path.join(dir, e.name)).size), 0);
console.log(`Site written to ${out} (${(size(out) / 1e6).toFixed(1)} MB)`);
