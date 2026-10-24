// Read-only PE32 scanner: exports, imports and ASCII strings.
// Usage: node tools/pe-scan.mjs <file.dll|exe> [exports|imports|strings [minLen] [regex]]
import fs from 'node:fs';

const [file, mode = 'exports', a1, a2] = process.argv.slice(2);
const b = fs.readFileSync(file);
const pe = b.readUInt32LE(0x3c);
const nsec = b.readUInt16LE(pe + 6);
const optSize = b.readUInt16LE(pe + 20);
const opt = pe + 24;
const imageBase = b.readUInt32LE(opt + 28);
const dd = (i) => ({ rva: b.readUInt32LE(opt + 96 + i * 8), size: b.readUInt32LE(opt + 100 + i * 8) });
const secs = [];
for (let i = 0; i < nsec; i++) {
  const s = opt + optSize + i * 40;
  secs.push({ name: b.toString('latin1', s, s + 8).replace(/\0.*/, ''), vsize: b.readUInt32LE(s + 8), va: b.readUInt32LE(s + 12), rsize: b.readUInt32LE(s + 16), raw: b.readUInt32LE(s + 20) });
}
const off = (rva) => {
  const s = secs.find((s) => rva >= s.va && rva < s.va + Math.max(s.vsize, s.rsize));
  return s ? rva - s.va + s.raw : -1;
};
const cstr = (o) => b.toString('latin1', o, b.indexOf(0, o));

if (mode === 'exports') {
  const e = dd(0);
  if (!e.rva) { console.log('no exports'); process.exit(0); }
  const o = off(e.rva);
  const n = b.readUInt32LE(o + 24), names = off(b.readUInt32LE(o + 32)), ords = off(b.readUInt32LE(o + 36)), funcs = off(b.readUInt32LE(o + 28));
  console.log(`dll=${cstr(off(b.readUInt32LE(o + 12)))} imageBase=0x${imageBase.toString(16)} exports=${n}`);
  for (let i = 0; i < n; i++) {
    const ord = b.readUInt16LE(ords + i * 2);
    console.log(`${cstr(off(b.readUInt32LE(names + i * 4)))} @ 0x${(imageBase + b.readUInt32LE(funcs + ord * 4)).toString(16)}`);
  }
  console.log('sections:', secs.map((s) => `${s.name}@0x${(imageBase + s.va).toString(16)}+${s.vsize}`).join(' '));
} else if (mode === 'imports') {
  let o = off(dd(1).rva);
  while (b.readUInt32LE(o + 12)) {
    const dll = cstr(off(b.readUInt32LE(o + 12)));
    let t = off(b.readUInt32LE(o) || b.readUInt32LE(o + 16));
    const fns = [];
    for (let v; (v = b.readUInt32LE(t)); t += 4) fns.push(v & 0x80000000 ? `#${v & 0xffff}` : cstr(off(v) + 2));
    console.log(`${dll} (${fns.length}): ${fns.join(' ')}`);
    o += 20;
  }
} else if (mode === 'strings') {
  const min = Number(a1 || 6);
  const re = a2 ? new RegExp(a2, 'i') : null;
  let start = -1;
  for (let i = 0; i <= b.length; i++) {
    const c = b[i];
    if (i < b.length && c >= 0x20 && c < 0x7f) { if (start < 0) start = i; continue; }
    if (start >= 0 && i - start >= min) {
      const s = b.toString('latin1', start, i);
      if (!re || re.test(s)) console.log(`0x${start.toString(16)}\t${s}`);
    }
    start = -1;
  }
}
