'use strict';

const https = require('https');
const http = require('http');

/**
 * Minimal JSON-RPC client for the Tari Ootle wallet daemon.
 */
class WalletClient {
  constructor(url = 'http://localhost:5100/json_rpc') {
    this.url = new URL(url);
    this.token = null;
    this._reqId = 1;
  }

  async authenticate() {
    const resp = await this._callRaw('auth.request', { credentials: 'None' });
    this.token = resp.result?.token ?? null;
  }

  async defaultAccount() {
    const result = await this._call('accounts.get_default', {});
    return {
      componentAddress: result.account.component_address,
      ownerPublicKey: result.account.owner_public_key,
    };
  }

  /**
   * Submit a manifest and wait for the result.
   * @param {string} manifest
   * @param {Record<string,string>} variables
   * @param {number} maxFee  microtari
   * @param {boolean} dryRun
   */
  async submitManifest(manifest, variables = {}, maxFee = 10_000, dryRun = false) {
    const submitResult = await this._call('transactions.submit_manifest', {
      manifest,
      variables,
      max_fee: maxFee,
      dry_run: dryRun,
    });
    const txId = submitResult.transaction_id;
    if (dryRun && submitResult.result) return submitResult.result;
    return this._waitResult(txId);
  }

  /** Forward an arbitrary JSON-RPC call from the dapp JS bridge. */
  async forward(method, params) {
    return this._call(method, params);
  }

  // ── internals ──────────────────────────────────────────────────────────────

  async _waitResult(txId) {
    return this._call('transactions.wait_result', {
      transaction_id: txId,
      timeout_secs: 120,
    });
  }

  async _call(method, params) {
    const raw = await this._callRaw(method, params);
    if (raw.error) throw new Error(`RPC ${method}: ${JSON.stringify(raw.error)}`);
    return raw.result;
  }

  _callRaw(method, params) {
    const body = JSON.stringify({
      jsonrpc: '2.0',
      id: this._reqId++,
      method,
      params,
    });

    const transport = this.url.protocol === 'https:' ? https : http;
    const options = {
      hostname: this.url.hostname,
      port: this.url.port || (this.url.protocol === 'https:' ? 443 : 80),
      path: this.url.pathname,
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Content-Length': Buffer.byteLength(body),
        ...(this.token ? { Authorization: `Bearer ${this.token}` } : {}),
      },
    };

    return new Promise((resolve, reject) => {
      const req = transport.request(options, (res) => {
        let data = '';
        res.on('data', (chunk) => (data += chunk));
        res.on('end', () => {
          try { resolve(JSON.parse(data)); }
          catch (e) { reject(new Error(`Invalid JSON from daemon: ${data}`)); }
        });
      });
      req.on('error', reject);
      req.write(body);
      req.end();
    });
  }
}

module.exports = { WalletClient };
