// Headless Chrome smoke test over the DevTools protocol (no npm dependencies).
// Usage: node tools/browser-check.mjs <url> <screenshot.png> [seconds=30] [key script...]
// Key script items: "wait:<s>", "key:<KeyCode>", "hold:<KeyCode>:<s>", "click:<x>:<y>", "stats",
//   "profile:<s>:<rows>", "files:<css selector>:<path>|<path>..." (file input), "until:<seconds>:<js expr>",
//   "text:<css selector>". ALICE_PROFILE=<dir> keeps the Chrome profile (OPFS saves) between runs.
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const [url, shot, secs = '30', ...script] = process.argv.slice(2);
const chrome = ['C:/Program Files/Google/Chrome/Application/chrome.exe', '/usr/bin/google-chrome']
  .find((p) => fs.existsSync(p));
const keepProfile = process.env.ALICE_PROFILE;
const profile = keepProfile || fs.mkdtempSync(path.join(os.tmpdir(), 'alice-chrome-'));
fs.mkdirSync(profile, { recursive: true });
const port = 9333;
const proc = spawn(chrome, [
  '--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`,
  '--window-size=1280,720', '--enable-unsafe-swiftshader', '--use-angle=d3d11', '--ignore-gpu-blocklist',
  '--enable-precise-memory-info', '--autoplay-policy=no-user-gesture-required', 'about:blank',
], { stdio: 'ignore' });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let ws;
for (let i = 0; i < 50 && !ws; i++) {
  await sleep(200);
  try {
    const list = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
    const page = list.find((t) => t.type === 'page');
    if (page) ws = new WebSocket(page.webSocketDebuggerUrl);
  } catch {}
}
await new Promise((r) => ws.addEventListener('open', r, { once: true }));
let id = 0;
const waiting = new Map();
const logs = [];
ws.addEventListener('message', (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && waiting.has(msg.id)) { waiting.get(msg.id)(msg); waiting.delete(msg.id); }
  if (msg.method === 'Runtime.consoleAPICalled') logs.push(`[${msg.params.type}] ${msg.params.args.map((a) => a.value ?? a.description).join(' ')}`);
  if (msg.method === 'Runtime.exceptionThrown') logs.push(`[exception] ${msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text}`);
});
const send = (method, params = {}) => new Promise((r) => { const n = ++id; waiting.set(n, r); ws.send(JSON.stringify({ id: n, method, params })); });
const evaluate = async (expr) => (await send('Runtime.evaluate', { expression: expr, returnByValue: true })).result?.result?.value;
const key = (type, code) => send('Input.dispatchKeyEvent', { type, code, key: code.replace(/^Key/, '').toLowerCase(), windowsVirtualKeyCode: code.startsWith('Key') ? code.charCodeAt(3) : { Enter: 13, Escape: 27, Space: 32, ArrowUp: 38, ArrowDown: 40, ArrowLeft: 37, ArrowRight: 39 }[code] });

await send('Runtime.enable');
await send('Page.enable');
const t0 = Date.now();
await send('Page.navigate', { url });
await sleep(Number(secs) * 1000);
await evaluate("document.getElementById('glcanvas')?.focus()");
for (const step of script) {
  const [op, a, b] = step.split(':');
  const rest = step.split(':').slice(2).join(':');
  if (op === 'files') {
    const { result: doc } = await send('DOM.getDocument');
    const { result: node } = await send('DOM.querySelector', { nodeId: doc.root.nodeId, selector: a });
    const r = await send('DOM.setFileInputFiles', { nodeId: node.nodeId, files: rest.split('|') });
    if (r.error) console.log('files error', JSON.stringify(r.error));
  }
  if (op === 'until') {
    const end = Date.now() + Number(a) * 1000;
    while (Date.now() < end && !(await evaluate(rest))) await sleep(500);
    console.log('until', rest, (await evaluate(rest)) ? 'met' : 'TIMED OUT', `after ${((Date.now() - t0) / 1000).toFixed(0)} s`);
  }
  if (op === 'eval') {
    const expr = step.slice(5);
    const r = await send('Runtime.evaluate', { expression: `(async () => { try { return JSON.stringify(await (${expr})); } catch (e) { return 'ERR ' + e; } })()`, awaitPromise: true, returnByValue: true, timeout: 20000 });
    console.log('eval', expr.slice(0, 80), '=>', r.result?.result?.value ?? JSON.stringify(r.error || r.result?.exceptionDetails));
  }
  if (op === 'text') console.log('text', a, JSON.stringify(await evaluate(`document.querySelector(${JSON.stringify(a)})?.textContent`)));
  if (op === 'wait') await sleep(Number(a) * 1000);
  if (op === 'stats') console.log('stats', new Date().toISOString().slice(11, 19), await evaluate("document.getElementById('hud')?.textContent || JSON.stringify(window.lgStats)"));
  if (op === 'profile') {
    await send('Profiler.enable');
    await send('Profiler.start');
    await sleep(Number(a) * 1000);
    const { result } = await send('Profiler.stop');
    const nodes = result.profile.nodes, self = new Map();
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const dt = result.profile.timeDeltas, samples = result.profile.samples;
    for (let i = 0; i < samples.length; i++) {
      const n = byId.get(samples[i]);
      const key = n.callFrame.functionName || '(' + n.callFrame.url.split('/').pop() + ')';
      self.set(key, (self.get(key) || 0) + (dt[i] || 0) / 1000);
    }
    const total = [...self.values()].reduce((x, y) => x + y, 0);
    console.log('profile total ms', total.toFixed(0));
    for (const [k, v] of [...self].sort((x, y) => y[1] - x[1]).slice(0, Number(b || 25))) console.log(v.toFixed(0).padStart(7), (100 * v / total).toFixed(1).padStart(5) + '%', k.slice(0, 150));
  }
  if (op === 'key') { await key('keyDown', a); await sleep(80); await key('keyUp', a); await sleep(300); }
  if (op === 'hold') { await key('keyDown', a); await sleep(Number(b) * 1000); await key('keyUp', a); }
  if (op === 'click') {
    await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: Number(a), y: Number(b) });
    await sleep(150);
    for (const type of ['mousePressed', 'mouseReleased']) await send('Input.dispatchMouseEvent', { type, x: Number(a), y: Number(b), button: 'left', clickCount: 1 });
    await sleep(300);
  }
}
const stats = await evaluate(`JSON.stringify({ fps: window.lgStats?.fps, hud: document.getElementById('hud')?.textContent,
  wasmMB: typeof wasm_memory !== 'undefined' ? Math.round(wasm_memory.buffer.byteLength / 1e6) : null,
  heapMB: performance.memory ? Math.round(performance.memory.usedJSHeapSize / 1e6) : null,
  stats: window.lgStats })`);
const img = await send('Page.captureScreenshot', { format: 'png' });
fs.writeFileSync(shot, Buffer.from(img.result.data, 'base64'));
console.log(`elapsed ${((Date.now() - t0) / 1000).toFixed(1)} s`);
console.log('stats', stats);
console.log(logs.slice(-40).join('\n'));
ws.close();
proc.kill();
await sleep(500);
if (!keepProfile) try { fs.rmSync(profile, { recursive: true, force: true }); } catch {}
process.exit(0);
