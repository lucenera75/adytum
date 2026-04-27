'use strict';

const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('arachne', {
  // Navigate to an ootle:// URL
  navigate: (url) => ipcRenderer.invoke('ootle:navigate', url),

  // Forward a wallet daemon call (method + params) from the dapp bridge
  call: (method, params) => ipcRenderer.invoke('ootle:call', { method, params }),

  // Config
  getConfig: () => ipcRenderer.invoke('ootle:getConfig'),
  setConfig: (updates) => ipcRenderer.invoke('ootle:setConfig', updates),

  // Subscribe to status events pushed from main (resolving, downloading, …)
  onStatus: (cb) => {
    ipcRenderer.on('ootle:status', (_e, data) => cb(data));
  },

  // Receive navigate commands pushed from main (deep links, open-url)
  onNavigate: (cb) => {
    ipcRenderer.on('ootle:navigate', (_e, url) => cb(url));
  },
});
