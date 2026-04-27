'use strict';

const { app, BrowserWindow, ipcMain, shell, protocol, session } = require('electron');
const path = require('path');
const { WalletClient } = require('./wallet');
const { parseOotleUrl, resolveHost } = require('./resolver');
const { loadBundle } = require('./bundle');

// ── Config (loaded from electron-store or env) ────────────────────────────────

const config = {
  daemonUrl: process.env.ADYTUM_DAEMON_URL ?? 'http://localhost:5100/json_rpc',
  registryAddress: process.env.ADYTUM_REGISTRY ?? null,
  maxFee: parseInt(process.env.ADYTUM_MAX_FEE ?? '10000', 10),
};

// ── Wallet client (one shared instance per session) ───────────────────────────

let walletClient = null;

async function getWallet() {
  if (!walletClient) {
    walletClient = new WalletClient(config.daemonUrl);
    await walletClient.authenticate();
  }
  return walletClient;
}

// ── Main window ───────────────────────────────────────────────────────────────

let mainWindow = null;

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 800,
    minHeight: 600,
    titleBarStyle: process.platform === 'darwin' ? 'hiddenInset' : 'default',
    backgroundColor: '#0f0f0f',
    webPreferences: {
      preload: path.join(__dirname, '..', 'preload', 'index.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false, // preload needs module access
    },
  });

  mainWindow.loadFile(path.join(__dirname, '..', 'renderer', 'index.html'));

  mainWindow.on('closed', () => { mainWindow = null; });
}

// ── ootle:// protocol — deep-link on macOS / Windows ─────────────────────────

app.setAsDefaultProtocolClient('ootle');

// macOS: open-url event fires when clicking an ootle:// link
app.on('open-url', (event, url) => {
  event.preventDefault();
  if (mainWindow) {
    mainWindow.webContents.send('ootle:navigate', url);
  }
});

// Windows / Linux: the URL is passed as a command-line argument
const gotLock = app.requestSingleInstanceLock();
if (!gotLock) {
  app.quit();
} else {
  app.on('second-instance', (_event, argv) => {
    const ootleArg = argv.find((a) => a.startsWith('ootle://'));
    if (ootleArg && mainWindow) {
      mainWindow.focus();
      mainWindow.webContents.send('ootle:navigate', ootleArg);
    }
  });
}

// ── App lifecycle ─────────────────────────────────────────────────────────────

app.whenReady().then(() => {
  // Block all network requests from sandboxed iframes except postMessage bridge
  session.defaultSession.webRequest.onBeforeRequest(
    { urls: ['*://*/*'] },
    (details, callback) => {
      const frame = details.frame;
      // Allow main renderer; block everything from sandboxed dapp iframes
      if (frame && frame.parent) {
        // Only allow data: URIs (already inlined) and local IPC; block real network
        callback({ cancel: true });
      } else {
        callback({});
      }
    }
  );

  createWindow();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});

// ── IPC: navigate to an ootle:// URL ─────────────────────────────────────────

ipcMain.handle('ootle:navigate', async (_event, rawUrl) => {
  try {
    const { host, path: urlPath } = parseOotleUrl(rawUrl);
    const wallet = await getWallet();

    // Resolve name → component address
    mainWindow?.webContents.send('ootle:status', { state: 'resolving', url: rawUrl });
    const componentAddress = await resolveHost(host, config.registryAddress, wallet);

    // Download, verify, and patch the bundle
    mainWindow?.webContents.send('ootle:status', { state: 'downloading', url: rawUrl });
    const { html, meta } = await loadBundle(componentAddress, wallet, config.maxFee);

    return { ok: true, html, meta, componentAddress, urlPath };
  } catch (err) {
    return { ok: false, error: err.message };
  }
});

// ── IPC: forward wallet API calls from the sandboxed dapp ────────────────────

ipcMain.handle('ootle:call', async (_event, { method, params }) => {
  try {
    const wallet = await getWallet();
    const result = await wallet.forward(method, params);
    return { ok: true, result };
  } catch (err) {
    return { ok: false, error: err.message };
  }
});

// ── IPC: settings ─────────────────────────────────────────────────────────────

ipcMain.handle('ootle:getConfig', () => ({ ...config }));

ipcMain.handle('ootle:setConfig', (_event, updates) => {
  Object.assign(config, updates);
  // Reset wallet client so it re-authenticates with the new URL
  walletClient = null;
  return { ok: true };
});
