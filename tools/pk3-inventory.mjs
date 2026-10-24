// Read-only inventory of Alice .pk3 (ZIP) archives.
// Usage: node tools/pk3-inventory.mjs <gameBaseDir> [--json out.json]
// Never writes into the game directory.
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';

export function readCentralDirectory(file) {
  const fd = fs.openSync(file, 'r');
  const size = fs.fstatSync(fd).size;
  const tailLen = Math.min(size, 65557);
  const tail = Buffer.alloc(tailLen);
  fs.readSync(fd, tail, 0, tailLen, size - tailLen);
  let eocd = -1;
  for (let i = tailLen - 22; i >= 0; i--) {
    if (tail.readUInt32LE(i) === 0x06054b50) { eocd = i; break; }
  }
  if (eocd < 0) throw new Error(`EOCD not found: ${file}`);
  const count = tail.readUInt16LE(eocd + 10);
  const cdSize = tail.readUInt32LE(eocd + 12);
  const cdOff = tail.readUInt32LE(eocd + 16);
  const cd = Buffer.alloc(cdSize);
  fs.readSync(fd, cd, 0, cdSize, cdOff);
  const entries = [];
  let p = 0;
  for (let i = 0; i < count; i++) {
    if (cd.readUInt32LE(p) !== 0x02014b50) throw new Error('bad CD entry');
    const method = cd.readUInt16LE(p + 10);
    const crc = cd.readUInt32LE(p + 16);
    const csize = cd.readUInt32LE(p + 20);
    const usize = cd.readUInt32LE(p + 24);
    const nlen = cd.readUInt16LE(p + 28);
    const xlen = cd.readUInt16LE(p + 30);
    const clen = cd.readUInt16LE(p + 32);
    const lho = cd.readUInt32LE(p + 42);
    const name = cd.toString('latin1', p + 46, p + 46 + nlen);
    entries.push({ name, method, crc, csize, usize, lho });
    p += 46 + nlen + xlen + clen;
  }
  return { fd, entries };
}

export function readEntry(fd, e) {
  const lh = Buffer.alloc(30);
  fs.readSync(fd, lh, 0, 30, e.lho);
  const start = e.lho + 30 + lh.readUInt16LE(26) + lh.readUInt16LE(28);
  const data = Buffer.alloc(e.csize);
  fs.readSync(fd, data, 0, e.csize, start);
  if (e.method === 0) return data;
  if (e.method === 8) return zlib.inflateRawSync(data);
  throw new Error(`unsupported method ${e.method}`);
}

function findPk3s(base) {
  const out = [];
  for (const d of [base, path.join(base, 'loc')]) {
    if (!fs.existsSync(d)) continue;
    for (const f of fs.readdirSync(d).sort()) if (/\.pk3$/i.test(f)) out.push(path.join(d, f));
  }
  return out;
}

function headerInfo(ext, buf) {
  const magic = buf.toString('latin1', 0, 4);
  const u32 = (o) => (buf.length >= o + 4 ? buf.readUInt32LE(o) : null);
  switch (ext) {
    case 'bsp': return { magic, version: u32(4) };
    case 'skb': case 'skd': case 'ska': case 'tan': case 'md3': case 'mdr':
      return { magic, version: u32(4) };
    case 'ftx': return { width: u32(0), height: u32(4), flag: u32(8) };
    case 'wav': return { magic, fmt: buf.length > 22 ? buf.readUInt16LE(20) : null, ch: buf.length > 24 ? buf.readUInt16LE(22) : null, rate: u32(24) };
    case 'tga': return { type: buf[2], w: buf.readUInt16LE(12), h: buf.readUInt16LE(14), bpp: buf[16] };
    default: return { magic: magic.replace(/[^\x20-\x7e]/g, '.') };
  }
}

const HEADER_EXTS = new Set(['bsp', 'skb', 'skd', 'ska', 'tan', 'md3', 'mdr', 'ftx', 'wav', 'tga', 'dcl', 'cin', 'roq', 'mp3']);

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(new URL(import.meta.url).pathname.replace(/^\/(\w:)/, '$1'))) {
  const base = process.argv[2];
  const jsonOut = process.argv.includes('--json') ? process.argv[process.argv.indexOf('--json') + 1] : null;
  const report = { pk3s: [] };
  for (const pk of findPk3s(base)) {
    const { fd, entries } = readCentralDirectory(pk);
    const byExt = {};
    const byTop = {};
    const headers = {};
    for (const e of entries) {
      if (e.name.endsWith('/')) continue;
      const ext = (e.name.match(/\.([^./]+)$/)?.[1] || '(none)').toLowerCase();
      const top = e.name.split('/')[0].toLowerCase();
      (byExt[ext] ??= { count: 0, bytes: 0 }).count++;
      byExt[ext].bytes += e.usize;
      (byTop[top] ??= { count: 0, bytes: 0 }).count++;
      byTop[top].bytes += e.usize;
      if (HEADER_EXTS.has(ext)) {
        const h = headers[ext] ??= { samples: [], distinct: {} };
        let info;
        try { info = headerInfo(ext, readEntry(fd, { ...e, csize: e.csize }).subarray(0, 64)); }
        catch (err) { info = { error: String(err.message) }; }
        const key = JSON.stringify(ext === 'ftx' || ext === 'tga' ? { flag: info.flag, bpp: info.bpp, type: info.type } : ext === 'wav' ? { fmt: info.fmt, rate: info.rate, ch: info.ch } : info);
        h.distinct[key] = (h.distinct[key] || 0) + 1;
        if (h.samples.length < 3) h.samples.push({ name: e.name, ...info });
      }
    }
    fs.closeSync(fd);
    report.pk3s.push({ file: path.relative(base, pk), entries: entries.length, byExt, byTop, headers,
      names: entries.map((e) => e.name) });
  }
  if (jsonOut) fs.writeFileSync(jsonOut, JSON.stringify(report, null, 1));
  for (const p of report.pk3s) {
    console.log(`\n== ${p.file}: ${p.entries} entries`);
    console.log(' top:', Object.entries(p.byTop).map(([k, v]) => `${k}=${v.count}`).join(' '));
    console.log(' ext:', Object.entries(p.byExt).sort((a, b) => b[1].count - a[1].count).map(([k, v]) => `${k}=${v.count}(${(v.bytes / 1e6).toFixed(1)}MB)`).join(' '));
    for (const [ext, h] of Object.entries(p.headers)) console.log(`  [${ext}] distinct:`, JSON.stringify(h.distinct), 'sample:', JSON.stringify(h.samples[0]));
  }
}
