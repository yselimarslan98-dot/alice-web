// Minimal S3 client (AWS Signature V4) for the player's own private bucket: Cloudflare R2,
// Backblaze B2, AWS S3 or any S3-compatible store. Runs in the browser and in Node 22+
// (tools/upload-s3.mjs) through Web Crypto. Path-style URLs: <endpoint>/<bucket>/<key>.
//   config: { endpoint, bucket, prefix?, region?, accessKeyId, secretAccessKey }

const te = new TextEncoder();
const hex = (buf) => [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, '0')).join('');
const sha256 = async (data) => hex(await crypto.subtle.digest('SHA-256', typeof data === 'string' ? te.encode(data) : data));
async function hmac(key, text) {
  const k = await crypto.subtle.importKey('raw', typeof key === 'string' ? te.encode(key) : key,
    { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']);
  return crypto.subtle.sign('HMAC', k, te.encode(text));
}
// RFC 3986 encoding as SigV4 requires it.
const enc = (s) => encodeURIComponent(s).replace(/[!'()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`);

export const EMPTY_SHA256 = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855';

// Returns the headers to send (Host is set by fetch itself but is part of the signature).
export async function signRequest({ method, url, headers = {}, payloadHash = 'UNSIGNED-PAYLOAD', region = 'auto',
  accessKeyId, secretAccessKey, date = new Date() }) {
  const u = new URL(url);
  const amzDate = date.toISOString().replace(/[-:]/g, '').replace(/\.\d{3}/, '');
  const day = amzDate.slice(0, 8);
  const all = { ...Object.fromEntries(Object.entries(headers).map(([k, v]) => [k.toLowerCase(), String(v).trim()])),
    host: u.host, 'x-amz-content-sha256': payloadHash, 'x-amz-date': amzDate };
  const names = Object.keys(all).sort();
  const query = [...u.searchParams].map(([k, v]) => [enc(k), enc(v)])
    .sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : a[1] < b[1] ? -1 : 1)).map(([k, v]) => `${k}=${v}`).join('&');
  const path = u.pathname.split('/').map((s) => enc(decodeURIComponent(s))).join('/');
  const canonical = [method, path, query, names.map((n) => `${n}:${all[n]}\n`).join(''), names.join(';'), payloadHash].join('\n');
  const scope = `${day}/${region}/s3/aws4_request`;
  const toSign = ['AWS4-HMAC-SHA256', amzDate, scope, await sha256(canonical)].join('\n');
  let key = await hmac(`AWS4${secretAccessKey}`, day);
  for (const part of [region, 's3', 'aws4_request']) key = await hmac(key, part);
  const signature = hex(await hmac(key, toSign));
  const out = { ...all };
  delete out.host;
  out.authorization = `AWS4-HMAC-SHA256 Credential=${accessKeyId}/${scope}, SignedHeaders=${names.join(';')}, Signature=${signature}`;
  return out;
}

const base = (c) => `${c.endpoint.replace(/\/+$/, '')}/${enc(c.bucket)}`;
const prefixOf = (c) => (c.prefix ? c.prefix.replace(/^\/+|\/+$/g, '') + '/' : '');
export const objectUrl = (c, name) => `${base(c)}/${(prefixOf(c) + name).split('/').map(enc).join('/')}`;

export async function s3fetch(c, method, url, { headers = {}, body, payloadHash } = {}) {
  const signed = await signRequest({ method, url, headers, payloadHash, region: c.region || 'auto',
    accessKeyId: c.accessKeyId, secretAccessKey: c.secretAccessKey });
  const r = await fetch(url, { method, headers: signed, body, duplex: body ? 'half' : undefined });
  if (!r.ok) {
    const text = await r.text().catch(() => '');
    const code = /<Code>([^<]*)<\/Code>/.exec(text)?.[1] || '';
    throw new Error(`${method} ${url.replace(/\?.*/, '')}: HTTP ${r.status} ${code}`.trim());
  }
  return r;
}

const unxml = (s) => s.replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(/&amp;/g, '&');

// Top-level objects under the prefix: [{ name, size }].
export async function listObjects(c) {
  const prefix = prefixOf(c);
  const out = [];
  let token = '';
  do {
    const url = new URL(`${base(c)}`);
    url.searchParams.set('list-type', '2');
    url.searchParams.set('delimiter', '/');
    if (prefix) url.searchParams.set('prefix', prefix);
    if (token) url.searchParams.set('continuation-token', token);
    const xml = await (await s3fetch(c, 'GET', url.href)).text();
    for (const m of xml.matchAll(/<Contents>([\s\S]*?)<\/Contents>/g)) {
      const key = unxml(/<Key>([^<]*)<\/Key>/.exec(m[1])[1]);
      out.push({ name: key.slice(prefix.length), size: Number(/<Size>(\d+)<\/Size>/.exec(m[1])[1]) });
    }
    token = /<IsTruncated>true<\/IsTruncated>/.test(xml) ? unxml(/<NextContinuationToken>([^<]*)</.exec(xml)?.[1] || '') : '';
  } while (token);
  return out;
}

export async function getObject(c, name) {
  return s3fetch(c, 'GET', objectUrl(c, name));
}

// Setup links carry the config in the URL fragment (never sent to any server).
export const encodeConfig = (c) => btoa(JSON.stringify(c)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
export function decodeConfig(text) {
  let c;
  try {
    c = JSON.parse(atob(text.replace(/-/g, '+').replace(/_/g, '/')));
  } catch {
    throw new Error('bağlantı eksik ya da bozuk; tamamını kopyaladığınızdan emin olun');
  }
  for (const k of ['endpoint', 'bucket', 'accessKeyId', 'secretAccessKey']) if (!c[k]) throw new Error(`eksik alan: ${k}`);
  return c;
}
