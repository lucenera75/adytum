'use strict';

const crypto = require('crypto');
const { unzipSync } = require('fflate');

const OOTLE_SDK_SHIM = `
<script id="__ootle_sdk__">
(function () {
  const pending = new Map();
  let _seq = 0;
  window.ootle = {
    call: function (request) {
      return new Promise(function (resolve, reject) {
        const id = 'ootle_' + (++_seq);
        pending.set(id, { resolve, reject });
        window.parent.postMessage({ __ootle: true, id, request }, '*');
      });
    }
  };
  window.addEventListener('message', function (e) {
    const msg = e.data;
    if (!msg || !msg.__ootle_resp) return;
    const cb = pending.get(msg.id);
    if (!cb) return;
    pending.delete(msg.id);
    if (msg.error) cb.reject(new Error(msg.error));
    else cb.resolve(msg.result);
  });
})();
</script>`;

/**
 * Download all chunks for a DappBundle, verify integrity, unzip, and return
 * the patched HTML ready to load in a sandboxed iframe.
 */
async function loadBundle(componentAddress, walletClient, maxFee = 10_000) {
  // 1. Fetch public manifest
  const manifestResult = await walletClient.submitManifest(
    `let b = global!("${componentAddress}"); let _m = b.get_manifest();`,
    {},
    1_000,
    true
  );
  const meta = extractManifest(manifestResult);
  if (!meta) throw new Error('Could not read bundle manifest');
  if (!meta.published) throw new Error('Bundle is not yet published');

  const { chunk_count: chunkCount, content_hash: contentHashHex, encrypted } = meta;

  // 2. Download chunks in parallel
  const chunkPromises = Array.from({ length: chunkCount }, (_, i) =>
    downloadChunk(componentAddress, i, walletClient, maxFee)
  );
  const chunkArrays = await Promise.all(chunkPromises);

  // 3. Reassemble
  const totalLen = chunkArrays.reduce((n, c) => n + c.length, 0);
  const assembled = new Uint8Array(totalLen);
  let offset = 0;
  for (const chunk of chunkArrays) {
    assembled.set(chunk, offset);
    offset += chunk.length;
  }

  // 4. Decrypt (not yet implemented — requires wallet daemon extension)
  let plaintext = assembled;
  if (encrypted) {
    throw new Error(
      'Encrypted bundles require wallet daemon decryption support (coming in a future release).'
    );
  }

  // 5. Verify SHA-256
  const actualHash = crypto
    .createHash('sha256')
    .update(Buffer.from(plaintext))
    .digest('hex');
  // contentHashHex may be an array of ints or a hex string depending on CBOR encoding
  const expectedHash = normaliseHash(contentHashHex);
  if (actualHash !== expectedHash) {
    throw new Error(
      `Bundle integrity check FAILED.\n  Expected: ${expectedHash}\n  Actual:   ${actualHash}`
    );
  }

  // 6. Unzip in memory
  const files = unzipSync(plaintext);

  // 7. Find entry point
  const adytumJson = files['adytum.json']
    ? JSON.parse(Buffer.from(files['adytum.json']).toString('utf8'))
    : null;
  const entryPoint = adytumJson?.entry_point ?? 'index.html';
  const entryBytes = files[entryPoint];
  if (!entryBytes) throw new Error(`Entry point '${entryPoint}' not found in bundle`);

  // 8. Patch HTML: inject ootle SDK shim, replace relative asset refs with data URIs
  let html = Buffer.from(entryBytes).toString('utf8');
  html = injectShim(html);
  html = replaceAssets(html, files);

  return { html, meta, entryPoint };
}

async function downloadChunk(componentAddress, index, walletClient, maxFee) {
  const manifest = `
let b = global!("${componentAddress}");
let _c = b.get_chunk(${index}u64);
`;
  const result = await walletClient.submitManifest(manifest, {}, maxFee, false);
  const bytes = extractBytes(result);
  if (!bytes) throw new Error(`Chunk ${index} returned no data`);
  return bytes;
}

// ── helpers ──────────────────────────────────────────────────────────────────

function extractManifest(result) {
  // Walk the result JSON looking for an object with 'chunk_count' and 'content_hash'
  function search(v) {
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      if ('chunk_count' in v && 'content_hash' in v) return v;
      for (const child of Object.values(v)) { const r = search(child); if (r) return r; }
    } else if (Array.isArray(v)) {
      for (const child of v) { const r = search(child); if (r) return r; }
    }
    return null;
  }
  return search(result);
}

function extractBytes(result) {
  // Look for a Bytes value in the result — either a Buffer, Uint8Array, or int array
  function search(v) {
    if (v instanceof Uint8Array || Buffer.isBuffer(v)) return new Uint8Array(v);
    if (Array.isArray(v) && v.length > 0 && typeof v[0] === 'number') {
      return new Uint8Array(v);
    }
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      for (const child of Object.values(v)) { const r = search(child); if (r) return r; }
    }
    return null;
  }
  return search(result);
}

function normaliseHash(h) {
  if (typeof h === 'string') return h.toLowerCase();
  if (Array.isArray(h)) return Buffer.from(h).toString('hex');
  return '';
}

function injectShim(html) {
  const headClose = html.indexOf('</head>');
  if (headClose !== -1) {
    return html.slice(0, headClose) + OOTLE_SDK_SHIM + html.slice(headClose);
  }
  return OOTLE_SDK_SHIM + html;
}

function replaceAssets(html, files) {
  return html.replace(/(src|href)="([^"#?]+)"/g, (match, attr, rawPath) => {
    const path = rawPath.replace(/^\.\//, '');
    const fileData = files[path];
    if (!fileData) return match;
    const mime = guessMime(path);
    const b64 = Buffer.from(fileData).toString('base64');
    return `${attr}="data:${mime};base64,${b64}"`;
  });
}

function guessMime(filename) {
  const ext = filename.split('.').pop().toLowerCase();
  return (
    { js: 'text/javascript', css: 'text/css', png: 'image/png',
      jpg: 'image/jpeg', jpeg: 'image/jpeg', gif: 'image/gif',
      svg: 'image/svg+xml', ico: 'image/x-icon', woff: 'font/woff',
      woff2: 'font/woff2', ttf: 'font/ttf', json: 'application/json',
    }[ext] ?? 'application/octet-stream'
  );
}

module.exports = { loadBundle };
