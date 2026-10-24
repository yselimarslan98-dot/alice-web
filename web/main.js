// Page side: game-data setup, the game worker, input forwarding, pointer lock, audio and HUD.
// Game data is never uploaded by the site: on a static host the PK3s are copied once into this
// site's private browser storage (OPFS), either from a folder the player picks or downloaded from
// the player's own private S3-compatible bucket (setup link); the local dev server reads them in place.
'use strict';

// All site data lives under one OPFS folder: GitHub Pages projects of one account share an origin.
const siteRoot = async () => (await navigator.storage.getDirectory()).getDirectoryHandle('alice-web', { create: true });

const $ = (id) => document.getElementById(id);
const canvas = $('glcanvas');
const hud = $('hud');

// ---------------------------------------------------------------- settings (this browser only)
// Frame cap (0: every display refresh), render scale and the statistics overlay. The fragment
// (#fps=N, #nohud, #hud) still overrides them for tests and shared links.
const SETTINGS_KEY = 'alice-web:settings';
const settings = { fps: 0, scale: 'auto', hud: 'off' };
try {
  Object.assign(settings, JSON.parse(localStorage.getItem(SETTINGS_KEY)) || {});
} catch {}
{
  const fps = location.hash.match(/fps=(\d+)/);
  if (fps) settings.fps = Number(fps[1]);
  if (/(^#|&)hud\b/.test(location.hash)) settings.hud = 'full';
  if (location.hash.includes('nohud')) settings.hud = 'off';
}
const saveSettings = () => {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
  } catch {}
};

// Device pixels per CSS pixel the game renders at. "auto" keeps full sharpness up to about a
// 1080p frame and scales down beyond it (4K and scaled high-DPI screens otherwise cost 2-4x).
function renderDpr() {
  const dpr = window.devicePixelRatio || 1;
  if (settings.scale !== 'auto') return dpr * Number(settings.scale);
  const pixels = canvas.clientWidth * canvas.clientHeight * dpr * dpr;
  return pixels > 1920 * 1080 ? dpr * Math.sqrt((1920 * 1080) / pixels) : dpr;
}

function bindSettings() {
  const fields = { fps: $('set-fps'), scale: $('set-scale'), hud: $('set-hud') };
  for (const [name, select] of Object.entries(fields)) {
    // A fragment value missing from the list (e.g. #fps=50) still shows as the current choice.
    if (![...select.options].some((o) => o.value === String(settings[name]))) select.add(new Option(String(settings[name])));
    select.value = String(settings[name]);
    select.addEventListener('change', () => {
      settings[name] = name === 'fps' ? Number(select.value) : select.value;
      saveSettings();
      applySettings();
    });
  }
  $('settings-button').addEventListener('click', () => { $('settings').hidden = !$('settings').hidden; });
  applySettings();
}

function applySettings() {
  hud.hidden = settings.hud === 'off';
  hud.classList.toggle('compact', settings.hud === 'fps');
  if (lastStats) showStats(lastStats);
  if (!worker) return;
  worker.postMessage({ type: 'fps', fps: settings.fps });
  worker.postMessage({ type: 'resize', ...cssSize() });
}

function show(id) {
  for (const el of document.querySelectorAll('.screen')) el.hidden = el.id !== id;
}
let fatalShown = false;
// Keeps the first error: the game reports its own message, then its exit traps in the worker.
function fatal(text) {
  if (fatalShown) return;
  fatalShown = true;
  $('error-text').textContent = text;
  show('error');
  if (document.pointerLockElement) document.exitPointerLock();
}

// ---------------------------------------------------------------- where the game data comes from
async function devServerPacks() {
  try {
    const r = await fetch('pk3/list.json', { cache: 'no-store' });
    return r.ok ? await r.json() : null;
  } catch {
    return null;
  }
}

async function packsDir() {
  const root = await siteRoot();
  return root.getDirectoryHandle('packs', { create: true });
}

async function storedPacks() {
  const names = [];
  for await (const [name, entry] of (await packsDir()).entries()) {
    if (entry.kind === 'file' && /\.pk3$/i.test(name)) names.push(name);
  }
  return names;
}

// The game mounts the top-level PK3s of Alice's bin/base folder (pak0.pk3 ... pak5_mod.pk3).
function pickBaseFiles(files) {
  const pak0 = files.find((f) => /(^|\/)pak0\.pk3$/i.test(f.webkitRelativePath || f.name));
  if (!pak0) return null;
  const dir = (pak0.webkitRelativePath || pak0.name).replace(/[^/]*$/, '');
  return files.filter((f) => {
    const path = f.webkitRelativePath || f.name;
    return /\.pk3$/i.test(path) && path.startsWith(dir) && !path.slice(dir.length).includes('/');
  });
}

// files: [{ name, size, stream() }] (File objects, or remote objects whose stream() is async).
async function copyToBrowser(files) {
  show('copying');
  // Ask the browser not to evict the copy; never wait on it (a prompt may stay unanswered).
  navigator.storage.persist?.().catch(() => {});
  const dir = await packsDir();
  const total = files.reduce((a, f) => a + f.size, 0);
  let done = 0;
  // pak0.pk3 marks a complete copy (setup checks for it), so it is written last: an OPFS
  // writable only replaces the file on close(), and an interrupted copy then simply restarts.
  const isPak0 = (f) => f.name.toLowerCase() === 'pak0.pk3';
  for (const file of [...files.filter((f) => !isPak0(f)), ...files.filter(isPak0)]) {
    const handle = await dir.getFileHandle(file.name, { create: true });
    const writable = await handle.createWritable();
    const reader = (await file.stream()).getReader();
    for (;;) {
      const { value, done: end } = await reader.read();
      if (end) break;
      await writable.write(value);
      done += value.length;
      $('copy-progress').value = done / total;
      $('copy-text').textContent = `${file.name} — ${(done / 1e6).toFixed(0)} / ${(total / 1e6).toFixed(0)} MB`;
    }
    await writable.close();
  }
}

// Local dev server only: copy saves from the earlier server-backed build (alice-web/userdata/)
// into browser storage, once, when browser storage holds no user data yet.
async function importLegacySaves() {
  try {
    const dir = await (await siteRoot()).getDirectoryHandle('userdata', { create: true });
    for await (const _ of dir.keys()) return; // already has user data
    const r = await fetch('legacy-userdata.json', { cache: 'no-store' });
    if (!r.ok) return;
    for (const rel of await r.json()) {
      const bytes = await (await fetch(`legacy-userdata/${rel}`, { cache: 'no-store' })).arrayBuffer();
      let target = dir;
      const parts = rel.split('/');
      for (const part of parts.slice(0, -1)) target = await target.getDirectoryHandle(part, { create: true });
      const writable = await (await target.getFileHandle(parts[parts.length - 1], { create: true })).createWritable();
      await writable.write(bytes);
      await writable.close();
    }
  } catch (e) {
    console.warn('Importing earlier saves failed:', e);
  }
}

// ---------------------------------------------------------------- the player's own bucket
// The bucket config (with a read-only key) arrives once through a setup link
// (#remote=<base64url JSON>, a URL fragment, so it reaches no server) or the setup form,
// and stays in this browser's OPFS so the packs can be fetched again if the browser evicts them.
async function readRemoteConfig() {
  try {
    const file = await (await (await siteRoot()).getFileHandle('remote.json')).getFile();
    return JSON.parse(await file.text());
  } catch {
    return null;
  }
}

async function saveRemoteConfig(config) {
  const writable = await (await (await siteRoot()).getFileHandle('remote.json', { create: true })).createWritable();
  await writable.write(JSON.stringify(config));
  await writable.close();
}

// Takes #remote=<config> or #k=<access code> out of the address bar (and out of the options
// passed to the game); returns the text to resolve, or null.
function takeLinkConfig() {
  const params = new URLSearchParams(location.hash.slice(1));
  const value = params.get('remote') ?? params.get('k');
  if (value === null) return null;
  const text = params.has('remote') ? `#remote=${value}` : value;
  params.delete('remote');
  params.delete('k');
  const rest = params.toString();
  history.replaceState(null, '', location.pathname + location.search + (rest ? `#${rest}` : ''));
  return text;
}

// Accepts a setup link (…#remote=…), a short link (…#k=…) or just the access code.
async function resolveConfig(text) {
  const s3 = await import('./s3.js');
  const link = /[#&]remote=([^&\s]+)/.exec(text);
  if (link) return s3.decodeConfig(link[1]);
  const code = /[#&]k=([^&\s]+)/.exec(text)?.[1] || text;
  const r = await fetch('bucket.sealed.json', { cache: 'no-store' });
  if (!r.ok) throw new Error('bu sitede şifreyle açılan bir depo tanımlı değil');
  return s3.unsealConfig(await r.json(), decodeURIComponent(code));
}

async function hasSealedBucket() {
  try {
    return (await fetch('bucket.sealed.json', { method: 'HEAD', cache: 'no-store' })).ok;
  } catch {
    return false;
  }
}

async function downloadFromBucket(config) {
  const s3 = await import('./s3.js');
  show('copying');
  $('copy-text').textContent = 'Depoya bağlanılıyor…';
  let objects;
  try {
    objects = await s3.listObjects(config);
  } catch (err) {
    throw new Error(`depo listelenemedi (${err.message}). Adresi, anahtarı ve depodaki CORS ayarını kontrol edin.`);
  }
  const packs = objects.filter((o) => /\.pk3$/i.test(o.name) && !o.name.includes('/'));
  if (!packs.some((o) => o.name.toLowerCase() === 'pak0.pk3')) throw new Error('depoda pak0.pk3 bulunamadı.');
  await copyToBrowser(packs.map((o) => ({ ...o, stream: async () => (await s3.getObject(config, o.name)).body })));
}

async function useBucket(config) {
  try {
    await downloadFromBucket(config);
  } catch (err) {
    return fatal(`Oyun dosyaları indirilemedi: ${err.message}`);
  }
  start('opfs');
}

async function clearBrowserData() {
  const root = await siteRoot();
  for (const name of ['packs', 'userdata', 'remote.json']) {
    try { await root.removeEntry(name, { recursive: true }); } catch {}
  }
}

async function setup() {
  if (!('storage' in navigator) || !navigator.storage.getDirectory || !('OffscreenCanvas' in window)) {
    return fatal('Bu tarayıcı gerekli özellikleri (OPFS, OffscreenCanvas) desteklemiyor. Güncel Chrome, Edge veya Firefox kullanın.');
  }
  const dev = await devServerPacks();
  if (dev) {
    await importLegacySaves();
    return start('http', dev);
  }
  const fromLink = takeLinkConfig();
  const haveData = (await storedPacks()).some((n) => n.toLowerCase() === 'pak0.pk3');
  let remote = await readRemoteConfig();
  if (fromLink) {
    try {
      remote = await resolveConfig(fromLink);
    } catch (err) {
      return fatal(`Kurulum bağlantısı açılamadı: ${err.message}`);
    }
    await saveRemoteConfig(remote);
  }
  if (haveData) return start('opfs');
  if (remote) return useBucket(remote);
  show('setup');
  if (await hasSealedBucket()) {
    $('link-text').textContent = 'Şifren varsa buraya yaz; oyun dosyaları kendi deponden bu tarayıcıya bir kez indirilir.';
    $('link-input').placeholder = 'Şifre';
    $('link-input').focus();
  }
  $('link-form').addEventListener('submit', async (e) => {
    e.preventDefault();
    const text = $('link-input').value.trim();
    if (!text) return;
    $('setup-error').textContent = 'Kontrol ediliyor…';
    let config;
    try {
      config = await resolveConfig(text);
    } catch (err) {
      $('setup-error').textContent = `Açılamadı: ${err.message}`;
      return;
    }
    await saveRemoteConfig(config);
    useBucket(config);
  });
  $('folder').addEventListener('change', async (e) => {
    const files = pickBaseFiles([...e.target.files]);
    if (!files) {
      $('setup-error').textContent = 'Seçilen klasörde pak0.pk3 bulunamadı. Alice kurulumundaki bin\\base klasörünü (ya da Alice klasörünü) seçin.';
      return;
    }
    try {
      await copyToBrowser(files);
    } catch (err) {
      return fatal(`Dosyalar kopyalanamadı: ${err}. Diskte yaklaşık 700 MB boş yer gerekir.`);
    }
    start('opfs');
  });
}

$('reset').addEventListener('click', async (e) => {
  e.preventDefault();
  if (!confirm('Bu sitede saklanan oyun dosyaları ve kayıtlar silinsin mi?')) return;
  worker?.terminate();
  await clearBrowserData();
  location.reload();
});

// ---------------------------------------------------------------- game worker
let worker = null;
let wantGrab = false;

function cssSize() {
  return { width: canvas.clientWidth, height: canvas.clientHeight, dpr: renderDpr() };
}

function start(source, packs) {
  show('game');
  const offscreen = canvas.transferControlToOffscreen();
  worker = new Worker('worker.js');
  worker.onmessage = onWorkerMessage;
  worker.onerror = (e) => fatal(`Oyun çalıştırılamadı: ${e.message}`);
  worker.postMessage({
    type: 'init',
    canvas: offscreen,
    size: cssSize(),
    source,
    packs,
    options: { search: location.search, hash: location.hash, fps: settings.fps },
  }, [offscreen]);
  bindSettings();
  bindInput();
  startAudio();
  pollGamepads();
}

function onWorkerMessage(message) {
  const m = message.data;
  switch (m.type) {
    case 'started':
      $('boot').hidden = true;
      break;
    case 'fatal':
      fatal(m.text);
      break;
    case 'console':
      (console[m.level] || console.log)('[game]', m.text);
      break;
    case 'grab':
      wantGrab = m.grab;
      if (!wantGrab && document.pointerLockElement) document.exitPointerLock();
      break;
    case 'fullscreen':
      if (!m.on && document.fullscreenElement) document.exitFullscreen().catch(() => {});
      if (m.on && !document.fullscreenElement) canvas.requestFullscreen().catch(() => {});
      break;
    case 'cursor':
      canvas.style.cursor = m.cursor;
      break;
    case 'audio':
      audio.requested = Math.max(0, audio.requested - m.blocks);
      if (m.pcm.length) scheduleAudio(m.pcm);
      break;
    case 'stats':
      showStats(m);
      break;
  }
}

// ---------------------------------------------------------------- input forwarding
const send = (target, name, event) => worker.postMessage({ type: 'event', target, name, event });
const mouse = (e) => {
  const r = canvas.getBoundingClientRect();
  return { clientX: e.clientX - r.left, clientY: e.clientY - r.top, movementX: e.movementX, movementY: e.movementY, button: e.button };
};
const key = (e) => ({ code: e.code, key: e.key, charCode: e.charCode, repeat: e.repeat, ctrlKey: e.ctrlKey, shiftKey: e.shiftKey, altKey: e.altKey });
// Keys the game uses that browsers would otherwise act on (F5 reload, Tab focus, page scroll...).
const gameKey = (e) => /^F([1-9]|1[0-2])$/.test(e.code) || ['Tab', 'Backspace', 'Space', 'ArrowUp', 'ArrowDown',
  'ArrowLeft', 'ArrowRight', 'Quote', 'Slash'].includes(e.code);

function bindInput() {
  canvas.addEventListener('mousemove', (e) => send('canvas', 'mousemove', mouse(e)));
  canvas.addEventListener('mousedown', (e) => {
    $('settings').hidden = true;
    canvas.focus();
    if (wantGrab && !document.pointerLockElement) {
      // Raw, unaccelerated motion like the native build's raw input.
      const p = canvas.requestPointerLock({ unadjustedMovement: true });
      if (p && p.catch) p.catch(() => canvas.requestPointerLock()?.catch?.(() => {}));
    }
    send('canvas', 'mousedown', mouse(e));
  });
  canvas.addEventListener('mouseup', (e) => send('canvas', 'mouseup', mouse(e)));
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    send('canvas', 'wheel', { deltaX: e.deltaX, deltaY: e.deltaY });
  }, { passive: false });
  canvas.addEventListener('contextmenu', (e) => e.preventDefault());
  window.addEventListener('keydown', (e) => {
    if (!$('settings').hidden && $('settings').contains(e.target)) return; // choosing in a list
    if (gameKey(e)) e.preventDefault();
    send('canvas', 'keydown', key(e));
  });
  window.addEventListener('keyup', (e) => send('canvas', 'keyup', key(e)));
  window.addEventListener('keypress', (e) => send('canvas', 'keypress', key(e)));
  document.addEventListener('pointerlockchange', () =>
    worker.postMessage({ type: 'pointerlock', locked: document.pointerLockElement === canvas }));
  const resized = () => worker.postMessage({ type: 'resize', ...cssSize() });
  new ResizeObserver(resized).observe(canvas);
  window.addEventListener('resize', resized);
  const focusChanged = () => worker.postMessage({ type: 'focus', focused: document.hasFocus() && !document.hidden });
  window.addEventListener('focus', focusChanged);
  window.addEventListener('blur', focusChanged);
  document.addEventListener('visibilitychange', focusChanged);
}

// ---------------------------------------------------------------- gamepads
// The game reads pads in XInput's layout (Looking Glass input.rs); browsers expose Xbox and
// PlayStation pads alike through the Gamepad API's standard mapping. Pads show up after their
// first button press (a browser privacy rule).
const PAD_BITS = [
  [0, 0x1000], [1, 0x2000], [2, 0x4000], [3, 0x8000], // A/Cross B/Circle X/Square Y/Triangle
  [4, 0x100], [5, 0x200], [8, 0x20], [9, 0x10], // LB/L1 RB/R1 Back/Create Start/Options
  [10, 0x40], [11, 0x80], [12, 1], [13, 2], [14, 4], [15, 8], // stick clicks, D-pad
];
const axis = (v) => Math.max(-32767, Math.min(32767, Math.round((v || 0) * 32767)));
let lastPads = '';

function readPad(p) {
  const b = (i) => p.buttons[i] || { pressed: false, value: 0 };
  let buttons = 0;
  for (const [i, bit] of PAD_BITS) if (b(i).pressed) buttons |= bit;
  return {
    buttons,
    lt: Math.round(b(6).value * 255),
    rt: Math.round(b(7).value * 255),
    // XInput's Y axes point up; the Gamepad API's point down.
    axes: [axis(p.axes[0]), -axis(p.axes[1]), axis(p.axes[2]), -axis(p.axes[3])],
    playstation: /054c|playstation|dualsense|dualshock|wireless controller/i.test(p.id),
  };
}

// The game stays on the first pad it finds, so it is shown only one: the one used last (with
// an Xbox and a PlayStation pad both connected, whichever is picked up just works).
let activePad = null;
const padActive = (s) => s.buttons || s.lt > 30 || s.rt > 30 || s.axes.some((v) => Math.abs(v) > 0.3 * 32767);

function pollGamepads() {
  const pads = [null, null, null, null];
  const seen = new Map();
  for (const p of navigator.getGamepads?.() || []) {
    if (p && p.connected) seen.set(p.index, readPad(p));
  }
  for (const [index, state] of seen) if (padActive(state) && index !== activePad) activePad = index;
  if (!seen.has(activePad)) activePad = seen.size ? seen.keys().next().value : null;
  if (activePad !== null) pads[0] = seen.get(activePad);
  const state = JSON.stringify(pads);
  if (state !== lastPads) {
    lastPads = state;
    // Sound may still wait for a first click or key; try a pad press too (browsers that do not
    // count it as user activation simply refuse).
    if (audio.ctx?.state === 'suspended' && pads.some((p) => p?.buttons)) audio.ctx.resume().catch(() => {});
    worker.postMessage({ type: 'gamepads', pads });
  }
  requestAnimationFrame(pollGamepads);
}

let toastTimer = 0;
function toast(text) {
  $('toast').textContent = text;
  $('toast').hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { $('toast').hidden = true; }, 5000);
}
window.addEventListener('gamepadconnected', (e) => {
  const name = e.gamepad.id.replace(/\s*\(.*$/, '') || 'kontrolcü';
  toast(`Kontrolcü algılandı: ${name}${e.gamepad.mapping === 'standard' ? '' : ' (standart olmayan düzen)'}`);
});
window.addEventListener('gamepaddisconnected', () => toast('Kontrolcü bağlantısı kesildi'));

// ---------------------------------------------------------------- audio
// The worker mixes 44.1 kHz stereo in Rust; we keep about LEAD seconds scheduled ahead.
const RATE = 44100, BLOCK = 1024, LEAD = 0.12;
const audio = { ctx: null, next: 0, requested: 0, underruns: 0, peak: 0, playing: false };
function startAudio() {
  audio.ctx = new AudioContext({ sampleRate: RATE, latencyHint: 'interactive' });
  const resume = () => { if (audio.ctx.state !== 'running') audio.ctx.resume(); };
  for (const ev of ['mousedown', 'keydown', 'touchstart']) window.addEventListener(ev, resume, { capture: true });
  setInterval(() => {
    const ctx = audio.ctx;
    if (ctx.state !== 'running') return;
    if (audio.next < ctx.currentTime) {
      if (audio.playing) audio.underruns++;
      audio.playing = false;
      audio.next = ctx.currentTime + 0.03;
    }
    const queued = audio.next - ctx.currentTime + audio.requested * BLOCK / RATE;
    const blocks = Math.ceil((LEAD - queued) * RATE / BLOCK);
    if (blocks > 0) {
      audio.requested += blocks;
      worker.postMessage({ type: 'audio-need', blocks });
    }
  }, 15);
}
function scheduleAudio(pcm) {
  const ctx = audio.ctx;
  const frames = pcm.length / 2;
  if (audio.next < ctx.currentTime) audio.next = ctx.currentTime + 0.03;
  const buf = ctx.createBuffer(2, frames, RATE);
  const left = buf.getChannelData(0), right = buf.getChannelData(1);
  let peak = 0;
  for (let i = 0; i < frames; i++) {
    left[i] = pcm[2 * i];
    right[i] = pcm[2 * i + 1];
    peak = Math.max(peak, Math.abs(left[i]), Math.abs(right[i]));
  }
  audio.peak = Math.max(audio.peak, peak);
  audio.playing = true;
  const node = ctx.createBufferSource();
  node.buffer = buf;
  node.connect(ctx.destination);
  node.start(audio.next);
  audio.next += frames / RATE;
}

// ---------------------------------------------------------------- HUD
let lastStats = null;
const escapeHtml = (text) => String(text).replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c]);

// Once a second from the worker. Always recorded (window.lgStats, tests read it); drawn only
// when the overlay is on.
function showStats(s) {
  if (s !== lastStats) {
    s.audio = `${audio.ctx?.state} · tepe ${audio.peak.toFixed(2)} · kesinti ${audio.underruns}`;
    audio.peak = 0;
  }
  window.lgStats = lastStats = s;
  if (settings.hud === 'off') return;
  const rate = s.fps >= 55 ? 'good' : s.fps >= 30 ? 'ok' : 'bad';
  const fps = `<span class="${rate}">${s.fps} fps</span>`;
  if (settings.hud === 'fps') {
    hud.innerHTML = fps;
    return;
  }
  const dpr = renderDpr();
  const width = Math.round(canvas.clientWidth * dpr), height = Math.round(canvas.clientHeight * dpr);
  const scale = Math.round((100 * dpr) / (window.devicePixelRatio || 1));
  const mem = performance.memory ? ` · JS ${(performance.memory.usedJSHeapSize / 1e6).toFixed(0)} MB` : '';
  const row = (label, text) => `<span class="label">${label.padEnd(11)}</span>${escapeHtml(text)}`;
  hud.innerHTML = [
    `${fps} <span class="label">· ${s.fps ? (1000 / s.fps).toFixed(1) : '-'} ms/kare · sınır ${settings.fps || 'ekran'}</span>`,
    row('Çözünürlük', `${width}×${height} (%${scale})`),
    row('Bellek', `wasm ${s.wasmMB} MB${mem}`),
    row('Çizim', `${s.draws} çağrı · ${s.uniforms} uniform (${s.skipped} atlandı) / kare`),
    s.sim !== undefined ? row('Simülasyon', `${s.sim} s/s · ${s.ticks} tick/s · hız ${s.speed} u/s`) : null,
    row('Ses', s.audio),
    row('Veri', `${s.source} · ${s.reads} okuma · ${s.readMB} MB · ${s.readS} s`),
    document.pointerLockElement ? row('Fare', 'kilitli') : null,
  ].filter(Boolean).join('\n');
}

setup();
