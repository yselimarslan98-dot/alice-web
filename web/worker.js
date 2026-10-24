// Game worker: runs miniquad's gl.js and the Looking Glass wasm on an OffscreenCanvas.
// Running off the page's main thread allows synchronous reads of the user's PK3s
// (OPFS sync access handles, or sync XHR range requests against the local dev server),
// which the game's blocking asset loader needs. The page (main.js) forwards input,
// owns pointer lock and plays the audio this worker mixes.
'use strict';

// All site data lives under one OPFS folder: GitHub Pages projects of one account share an origin.
const siteRoot = async () => (await navigator.storage.getDirectory()).getDirectoryHandle('alice-web', { create: true });

const post = (message, transfer) => self.postMessage(message, transfer || []);

for (const level of ['log', 'info', 'warn', 'error']) {
  const original = console[level].bind(console);
  console[level] = (...args) => {
    original(...args);
    post({ type: 'console', level, text: args.map(String).join(' ') });
  };
}

self.requestAnimationFrame = self.requestAnimationFrame || ((cb) => setTimeout(() => cb(performance.now()), 1000 / 60));
self.cancelAnimationFrame = self.cancelAnimationFrame || clearTimeout;

// ---------------------------------------------------------------- DOM stand-ins for gl.js
const listeners = { canvas: {}, window: {}, document: {} };
let focused = true;
let options = { search: '', hash: '' };

function installDom(canvas, size) {
  self.window = self;
  self.devicePixelRatio = size.dpr;
  canvas.clientWidth = size.width;
  canvas.clientHeight = size.height;
  canvas.style = {};
  canvas.focus = () => {};
  canvas.getBoundingClientRect = () => ({ left: 0, top: 0, width: canvas.clientWidth, height: canvas.clientHeight });
  canvas.requestPointerLock = () => post({ type: 'grab', grab: true });
  canvas.requestFullscreen = () => post({ type: 'fullscreen', on: true });
  canvas.addEventListener = (type, fn) => (listeners.canvas[type] ||= []).push(fn);
  self.document = {
    querySelector: () => canvas,
    hasFocus: () => focused,
    addEventListener: (type, fn) => (listeners.document[type] ||= []).push(fn),
    exitPointerLock: () => post({ type: 'grab', grab: false }),
    exitFullscreen: () => post({ type: 'fullscreen', on: false }),
    pointerLockElement: null,
    fullscreenElement: null,
    visibilityState: 'visible',
  };
  const addNative = self.addEventListener.bind(self);
  self.addEventListener = (type, fn, opts) =>
    type === 'message' ? addNative(type, fn, opts) : (listeners.window[type] ||= []).push(fn);
  self.alert = (message) => console.error(message);
}

function dispatch(target, name, data) {
  const event = Object.assign({ preventDefault() {}, stopPropagation() {} }, data);
  const object = target === 'canvas' ? canvas : target === 'window' ? self : self.document;
  const handler = object['on' + name];
  if (typeof handler === 'function') handler(event);
  for (const fn of listeners[target][name] || []) fn(event);
}

// ---------------------------------------------------------------- game data and user data
const packs = new Map(); // lower-case name -> { name, size, handle? }
let packSource = 'opfs';
const stats = { reads: 0, readBytes: 0, readMs: 0 };
const userData = new Map(); // '/userdata/<path>' -> Uint8Array

async function openOpfsPacks() {
  const root = await siteRoot();
  const dir = await root.getDirectoryHandle('packs', { create: true });
  for await (const [name, entry] of dir.entries()) {
    if (entry.kind !== 'file' || !/\.pk3$/i.test(name)) continue;
    const handle = await entry.createSyncAccessHandle();
    packs.set(name.toLowerCase(), { name, size: handle.getSize(), handle });
  }
}

async function loadUserData() {
  const root = await siteRoot();
  const walk = async (dir, prefix) => {
    for await (const [name, entry] of dir.entries()) {
      if (entry.kind === 'directory') await walk(entry, `${prefix}${name}/`);
      else userData.set(`/userdata/${prefix}${name}`, new Uint8Array(await (await entry.getFile()).arrayBuffer()));
    }
  };
  await walk(await root.getDirectoryHandle('userdata', { create: true }), '');
}

// Saves are written synchronously into memory for the game, then persisted to OPFS in order.
let persisting = Promise.resolve();
function persistUserData(url, bytes) {
  persisting = persisting.then(async () => {
    const parts = url.slice('/userdata/'.length).split('/');
    let dir = await (await siteRoot()).getDirectoryHandle('userdata', { create: true });
    for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part, { create: true });
    const file = await dir.getFileHandle(parts[parts.length - 1], { create: true });
    const writable = await file.createWritable();
    await writable.write(bytes);
    await writable.close();
  }).catch((e) => console.error('lgweb: saving', url, 'failed:', e));
}

function readPack(name, offset, n, dst) {
  const pack = packs.get(name.toLowerCase());
  if (!pack) return -1;
  const t0 = performance.now();
  let got;
  if (pack.handle) {
    got = pack.handle.read(new Uint8Array(wasm_memory.buffer, dst, n), { at: offset });
  } else {
    const xhr = new XMLHttpRequest();
    xhr.open('GET', `pk3/${encodeURIComponent(pack.name)}`, false);
    xhr.responseType = 'arraybuffer';
    xhr.setRequestHeader('Range', `bytes=${offset}-${offset + n - 1}`);
    xhr.send(null);
    if (xhr.status !== 206 && xhr.status !== 200) return -1;
    const bytes = new Uint8Array(xhr.response, 0, Math.min(n, xhr.response.byteLength));
    new Uint8Array(wasm_memory.buffer, dst, bytes.length).set(bytes);
    got = bytes.length;
  }
  stats.reads++;
  stats.readBytes += got;
  stats.readMs += performance.now() - t0;
  return got;
}

// ---------------------------------------------------------------- GL fast paths (WebGL2)
// miniquad re-uploads every uniform of a material before each draw, mostly unchanged;
// uniform values persist per program and each location id belongs to one program,
// so uploads matching the last value sent to that location are skipped.
const glCounts = { draws: 0, uniforms: 0, skipped: 0 };
function installFastGl(env) {
  let heap = null, f32 = null, i32 = null, u8 = null;
  const views = () => {
    if (heap !== wasm_memory.buffer) {
      heap = wasm_memory.buffer;
      f32 = new Float32Array(heap);
      i32 = new Int32Array(heap);
      u8 = new Uint8Array(heap);
    }
  };
  const last = new Map();
  const changed = (loc, src, start, len) => {
    let prev = last.get(loc);
    if (prev && prev.length === len) {
      let same = true;
      for (let i = 0; i < len; i++) {
        if (prev[i] !== src[start + i]) { same = false; break; }
      }
      if (same) return false;
    } else {
      prev = new Float64Array(len);
      last.set(loc, prev);
    }
    for (let i = 0; i < len; i++) prev[i] = src[start + i];
    return true;
  };
  let webgl2 = null;
  const fast = (name, impl) => {
    const original = env[name];
    env[name] = function (a, b, c, d) {
      if (webgl2 === null) webgl2 = gl instanceof WebGL2RenderingContext;
      return webgl2 ? impl(a, b, c, d) : original(a, b, c, d);
    };
  };
  for (const [n, w] of [[1, 'uniform1fv'], [2, 'uniform2fv'], [3, 'uniform3fv'], [4, 'uniform4fv']]) {
    fast('glU' + w.slice(1), (loc, count, ptr) => {
      views();
      if (changed(loc, f32, ptr >> 2, n * count)) gl[w](GL.uniforms[loc], f32, ptr >> 2, n * count);
      else glCounts.skipped++;
    });
  }
  for (const [n, w] of [[1, 'uniform1iv'], [2, 'uniform2iv'], [3, 'uniform3iv'], [4, 'uniform4iv']]) {
    fast('glU' + w.slice(1), (loc, count, ptr) => {
      views();
      if (changed(loc, i32, ptr >> 2, n * count)) gl[w](GL.uniforms[loc], i32, ptr >> 2, n * count);
      else glCounts.skipped++;
    });
  }
  fast('glUniformMatrix4fv', (loc, count, transpose, ptr) => {
    views();
    // Transposed and plain uploads of the same numbers differ; never mix them in the cache.
    if (transpose || changed(loc, f32, ptr >> 2, 16 * count)) {
      gl.uniformMatrix4fv(GL.uniforms[loc], !!transpose, f32, ptr >> 2, 16 * count);
    } else {
      glCounts.skipped++;
    }
  });
  // Any other uniform setter (glUniform1f, glUniform1i...) bypasses the cache: forget that location.
  const cached = new Set(['glUniform1fv', 'glUniform2fv', 'glUniform3fv', 'glUniform4fv', 'glUniform1iv',
    'glUniform2iv', 'glUniform3iv', 'glUniform4iv', 'glUniformMatrix4fv']);
  for (const name of Object.keys(env).filter((n) => /^glUniform/.test(n) && !cached.has(n))) {
    const inner = env[name];
    env[name] = function (loc, b, c, d) {
      last.delete(loc);
      return inner(loc, b, c, d);
    };
  }
  fast('glBufferSubData', (target, offset, size, ptr) => {
    views();
    gl.bufferSubData(target, offset, u8, ptr, size);
  });
  // macroquad flushes and finishes after every frame; the browser presents the canvas anyway.
  env.glFinish = function () {};
  for (const name of ['glDrawElements', 'glDrawElementsInstanced', 'glDrawArrays', 'glDrawArraysInstanced']) {
    const inner = env[name];
    if (!inner) continue;
    env[name] = function (a, b, c, d, e) {
      glCounts.draws++;
      return inner(a, b, c, d, e);
    };
  }
  for (const name of Object.keys(env).filter((n) => /^glUniform/.test(n))) {
    const inner = env[name];
    env[name] = function (a, b, c, d) {
      glCounts.uniforms++;
      return inner(a, b, c, d);
    };
  }
}

// ---------------------------------------------------------------- Looking Glass bridge plugin
let pending = null;
let gameOver = false; // after a fatal error the wasm event handler is gone
const encoder = new TextEncoder();
// Latest gamepad state from the page (the Gamepad API is not available in workers).
let gamepads = [];

const decoder = new TextDecoder();
const text = (ptr, len) => decoder.decode(new Uint8Array(wasm_memory.buffer, ptr, len).slice());

const lgweb = {
  name: 'lgvfs',
  version: 1,
  register_plugin(importObject) {
    const env = importObject.env;
    installFastGl(env);
    env.lgvfs_fetch = (ptr, len) => {
      const url = text(ptr, len);
      if (url === 'lgweb:query') pending = encoder.encode(decodeURIComponent(options.search.replace(/^\?/, '').replace(/\+/g, ' ')));
      else if (url === 'lgweb:packs') pending = encoder.encode([...packs.values()].map((p) => p.name).join('\n'));
      else pending = userData.get(url) || null;
      return pending ? pending.length : -1;
    };
    env.lgvfs_take = (dst) => {
      new Uint8Array(wasm_memory.buffer, dst, pending.length).set(pending);
      pending = null;
    };
    env.lgweb_put = (ptr, len, body, bodyLen) => {
      const url = text(ptr, len);
      if (!url.startsWith('/userdata/')) return 403;
      const bytes = new Uint8Array(wasm_memory.buffer, body, bodyLen).slice();
      userData.set(url, bytes);
      persistUserData(url, bytes);
      return 200;
    };
    env.lgweb_pack_len = (ptr, len) => {
      const pack = packs.get(text(ptr, len).toLowerCase());
      return pack ? pack.size : -1;
    };
    env.lgweb_pack_read = (ptr, len, offset, dst, n) => readPack(text(ptr, len), offset, n, dst);
    env.lgweb_fatal = (ptr, len) => {
      gameOver = true;
      post({ type: 'fatal', text: `Oyun durdu: ${text(ptr, len)}` });
    };
    // Gamepads, in XInput's layout (input.rs RawPad: u16 buttons, u8 lt, u8 rt, i16 lx ly rx ry).
    env.lgweb_pad = (index, dst) => {
      const pad = gamepads[index];
      if (!pad) return 0;
      const view = new DataView(wasm_memory.buffer, dst, 12);
      view.setUint16(0, pad.buttons, true);
      view.setUint8(2, pad.lt);
      view.setUint8(3, pad.rt);
      pad.axes.forEach((v, i) => view.setInt16(4 + 2 * i, v, true));
      return 1;
    };
    env.lgweb_pad_style = (index) => (gamepads[index]?.playstation ? 1 : 0);
    env.sapp_set_cursor_grab = (grab) => post({ type: 'grab', grab: !!grab });
    env.sapp_set_fullscreen = (on) => post({ type: 'fullscreen', on: !!on });
    env.sapp_set_cursor = (ptr, len) => post({ type: 'cursor', cursor: text(ptr, len) });
    // Looking Glass calls glDepthMask directly; miniquad 0.4.8's gl.js does not provide it.
    env.glDepthMask = (flag) => gl.depthMask(!!flag);
  },
  on_init() {
    // Frame cap (default 60; page fragment #fps=30, #fps=0 for uncapped). The simulation uses
    // real elapsed time, so skipping display refreshes never changes game speed.
    const match = options.hash.match(/fps=(\d+)/);
    const cap = match ? Number(match[1]) : 60;
    const interval = cap > 0 ? 1000 / cap : 0;
    const animate = self.animation;
    let due = 0;
    self.animation = function (t) {
      const now = performance.now();
      if (interval) {
        if (due === 0) due = now;
        if (now < due - 1) {
          animation_frame_timeout = self.requestAnimationFrame(self.animation);
          return;
        }
        due += interval;
        if (due < now - interval) due = now; // resynchronise after a long (loading) frame
      }
      gameFrames++;
      return animate(t);
    };
    setInterval(sendStats, 1000);
  },
};

// ---------------------------------------------------------------- audio and diagnostics
const AUDIO_BLOCK = 1024;
// Always answers, even with no samples (before the game starts its mixer), so the page's
// count of outstanding blocks never gets stuck.
function fillAudio(blocks) {
  let filled = 0;
  const pcm = new Float32Array(blocks * AUDIO_BLOCK * 2);
  if (typeof wasm_exports !== 'undefined' && wasm_exports.lgweb_audio_fill) {
    for (; filled < blocks; filled++) {
      const ptr = wasm_exports.lgweb_audio_fill(AUDIO_BLOCK);
      if (!ptr) break;
      pcm.set(new Float32Array(wasm_memory.buffer, ptr, AUDIO_BLOCK * 2), filled * AUDIO_BLOCK * 2);
    }
  }
  const out = pcm.subarray(0, filled * AUDIO_BLOCK * 2).slice();
  post({ type: 'audio', pcm: out, blocks }, [out.buffer]);
}

let gameFrames = 0;
let lastSim = null;
function sendStats() {
  const report = {
    type: 'stats',
    fps: gameFrames,
    wasmMB: typeof wasm_memory !== 'undefined' ? Math.round(wasm_memory.buffer.byteLength / 1e6) : 0,
    draws: gameFrames ? Math.round(glCounts.draws / gameFrames) : 0,
    uniforms: gameFrames ? Math.round(glCounts.uniforms / gameFrames) : 0,
    skipped: gameFrames ? Math.round(glCounts.skipped / gameFrames) : 0,
    reads: stats.reads,
    readMB: Math.round(stats.readBytes / 1e6),
    readS: +(stats.readMs / 1000).toFixed(1),
    source: packSource,
  };
  if (typeof wasm_exports !== 'undefined' && wasm_exports.lgweb_debug_sim) {
    const now = [0, 1, 2, 3].map((i) => wasm_exports.lgweb_debug_sim(i));
    if (lastSim) {
      report.sim = +(now[0] - lastSim[0]).toFixed(2);
      report.ticks = now[1] - lastSim[1];
      report.speed = Math.round(Math.hypot(now[2] - lastSim[2], now[3] - lastSim[3]));
    }
    lastSim = now;
  }
  post(report);
  gameFrames = 0;
  glCounts.draws = glCounts.uniforms = glCounts.skipped = 0;
}

// ---------------------------------------------------------------- messages from the page
self.onmessage = async (message) => {
  const m = message.data;
  if (gameOver && m.type !== 'audio-need') return;
  switch (m.type) {
    case 'init': {
      options = m.options;
      installDom(m.canvas, m.size);
      try {
        if (m.source === 'http') {
          packSource = 'http';
          for (const p of m.packs) packs.set(p.name.toLowerCase(), { name: p.name, size: p.size });
        } else {
          await openOpfsPacks();
        }
        await loadUserData();
      } catch (e) {
        post({ type: 'fatal', text: `Oyun verisi açılamadı: ${e}` });
        return;
      }
      if (!packs.has('pak0.pk3')) {
        post({ type: 'fatal', text: 'pak0.pk3 bulunamadı.' });
        return;
      }
      importScripts('vendor/miniquad-gl.js');
      miniquad_add_plugin(lgweb);
      load('wasm/looking-glass.wasm');
      post({ type: 'started' });
      break;
    }
    case 'event':
      dispatch(m.target, m.name, m.event);
      break;
    case 'resize':
      canvas.clientWidth = m.width;
      canvas.clientHeight = m.height;
      self.devicePixelRatio = m.dpr;
      if (typeof self.onresize === 'function') self.onresize();
      break;
    case 'focus':
      focused = m.focused;
      for (const fn of listeners.window[m.focused ? 'focus' : 'blur'] || []) fn({});
      break;
    case 'pointerlock':
      self.document.pointerLockElement = m.locked ? canvas : null;
      break;
    case 'audio-need':
      fillAudio(m.blocks);
      break;
    case 'gamepads':
      gamepads = m.pads;
      break;
  }
};
