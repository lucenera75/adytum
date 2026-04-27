'use strict';

/**
 * Parse an ootle:// URL into its components and resolve the name to a
 * DappBundle component address via the DappRegistry if needed.
 */

function parseOotleUrl(rawUrl) {
  // Normalise: ootle://name/path?query  OR  ootle://component_hex.../
  let url;
  try {
    // URL() doesn't handle custom schemes well; convert to http for parsing
    url = new URL(rawUrl.replace(/^ootle:\/\//, 'http://ootle-dummy/'));
  } catch {
    throw new Error(`Invalid ootle:// URL: ${rawUrl}`);
  }
  const host = url.hostname; // name or "component_hex"
  const path = url.pathname === '/' ? '' : url.pathname;
  const query = url.search;
  return { host, path, query };
}

/**
 * Resolve host → component address.
 * If host looks like a component address, return it directly.
 * Otherwise, query the DappRegistry.
 */
async function resolveHost(host, registryAddress, walletClient) {
  if (host.startsWith('component_')) return host;
  if (!registryAddress) {
    throw new Error(
      `Cannot resolve name '${host}': no DappRegistry address configured.\n` +
        'Set the registry address in Arachne settings.'
    );
  }

  const manifest = `
let registry = global!("${registryAddress}");
let _addr = registry.resolve("${host}");
`;
  const result = await walletClient.submitManifest(manifest, {}, 1_000, true);
  const addr = findComponentAddress(result);
  if (!addr) throw new Error(`Name '${host}' not found in registry`);
  return addr;
}

function findComponentAddress(obj) {
  if (typeof obj === 'string' && obj.startsWith('component_')) return obj;
  if (Array.isArray(obj)) {
    for (const v of obj) { const r = findComponentAddress(v); if (r) return r; }
  } else if (obj && typeof obj === 'object') {
    for (const v of Object.values(obj)) { const r = findComponentAddress(v); if (r) return r; }
  }
  return null;
}

module.exports = { parseOotleUrl, resolveHost };
