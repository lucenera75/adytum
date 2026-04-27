'use strict';

// ── Elements ──────────────────────────────────────────────────────────────────
const addressInput  = document.getElementById('address-input');
const statusDot     = document.getElementById('status-dot');
const overlay       = document.getElementById('overlay');
const overlayIcon   = overlay.querySelector('.overlay-icon');
const overlayTitle  = overlay.querySelector('.overlay-title');
const overlayBody   = overlay.querySelector('.overlay-body');
const sandbox       = document.getElementById('sandbox');
const progressBar   = document.getElementById('progress-bar');
const btnBack       = document.getElementById('btn-back');
const btnForward    = document.getElementById('btn-forward');
const settingsBtn   = document.getElementById('settings-btn');
const settingsPanel = document.getElementById('settings-panel');

// ── History ───────────────────────────────────────────────────────────────────
const history = { stack: [], index: -1 };

function historyPush(url) {
  history.stack = history.stack.slice(0, history.index + 1);
  history.stack.push(url);
  history.index = history.stack.length - 1;
  updateNavButtons();
}

function updateNavButtons() {
  btnBack.disabled    = history.index <= 0;
  btnForward.disabled = history.index >= history.stack.length - 1;
}

btnBack.addEventListener('click', () => {
  if (history.index > 0) navigate(history.stack[--history.index], false);
});
btnForward.addEventListener('click', () => {
  if (history.index < history.stack.length - 1) navigate(history.stack[++history.index], false);
});

// ── Navigation ────────────────────────────────────────────────────────────────
async function navigate(rawInput, pushHistory = true) {
  let url = rawInput.trim();
  if (!url) return;
  if (!url.startsWith('ootle://')) url = 'ootle://' + url;

  addressInput.value = url.replace(/^ootle:\/\//, '');
  if (pushHistory) historyPush(url);

  setStatus('loading', 'Resolving…');
  setProgress(0.1);
  showOverlay('🕸', 'Loading dapp…', url.replace(/^ootle:\/\//, ''), false);

  const result = await window.arachne.navigate(url);

  if (!result.ok) {
    setProgress(0);
    setStatus('error', result.error);
    showOverlay('⚠', 'Failed to load dapp', result.error, true);
    return;
  }

  setProgress(0.9);

  // Load the patched HTML into the sandboxed iframe via srcdoc
  sandbox.srcdoc = result.html;
  sandbox.classList.add('visible');
  overlay.classList.add('hidden');

  setProgress(1);
  setTimeout(() => setProgress(0), 500);
  setStatus('ok', `${result.meta?.name ?? ''} v${result.meta?.version ?? ''}`.trim());

  document.title = `${result.meta?.name ?? url} — Arachne`;
}

addressInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') navigate(addressInput.value);
});

// ── postMessage bridge ────────────────────────────────────────────────────────
// Relay ootle.call() messages from the sandboxed iframe to the wallet daemon.
window.addEventListener('message', async (e) => {
  const msg = e.data;
  if (!msg || !msg.__ootle) return;

  const { id, request } = msg;
  const { method, params } = request ?? {};

  const resp = await window.arachne.call(method, params ?? {});

  sandbox.contentWindow?.postMessage(
    { __ootle_resp: true, id, result: resp.result, error: resp.error ?? null },
    '*'
  );
});

// ── Status helpers ────────────────────────────────────────────────────────────
function setStatus(state, label) {
  statusDot.className = '';
  statusDot.classList.add(state);
  statusDot.title = label;
}

function setProgress(frac) {
  if (frac <= 0) {
    progressBar.classList.remove('active');
    progressBar.style.transform = 'scaleX(0)';
  } else {
    progressBar.classList.add('active');
    progressBar.style.transform = `scaleX(${frac})`;
  }
}

function showOverlay(icon, title, body, isError) {
  overlayIcon.textContent = icon;
  overlayTitle.textContent = title;
  overlayBody.textContent = body;
  overlayBody.className = 'overlay-body' + (isError ? ' error' : '');
  overlay.classList.remove('hidden');
  sandbox.classList.remove('visible');
}

// ── Status events from main process ──────────────────────────────────────────
window.arachne.onStatus(({ state, url }) => {
  if (state === 'resolving')   { setStatus('loading', 'Resolving name…'); setProgress(0.2); }
  if (state === 'downloading') { setStatus('loading', 'Downloading bundle…'); setProgress(0.5); }
});

// ── Deep-link from main process (open-url / second-instance) ─────────────────
window.arachne.onNavigate((url) => navigate(url));

// ── Settings panel ────────────────────────────────────────────────────────────
settingsBtn.addEventListener('click', async () => {
  if (settingsPanel.classList.toggle('open')) {
    const cfg = await window.arachne.getConfig();
    document.getElementById('cfg-daemon-url').value = cfg.daemonUrl ?? '';
    document.getElementById('cfg-registry').value   = cfg.registryAddress ?? '';
    document.getElementById('cfg-max-fee').value    = cfg.maxFee ?? 10000;
  }
});

document.getElementById('cfg-save').addEventListener('click', async () => {
  await window.arachne.setConfig({
    daemonUrl:       document.getElementById('cfg-daemon-url').value.trim() || null,
    registryAddress: document.getElementById('cfg-registry').value.trim()   || null,
    maxFee:          parseInt(document.getElementById('cfg-max-fee').value, 10) || 10_000,
  });
  settingsPanel.classList.remove('open');
  setStatus('ok', 'Settings saved');
});

// Close settings when clicking outside
document.addEventListener('click', (e) => {
  if (!settingsPanel.contains(e.target) && e.target !== settingsBtn) {
    settingsPanel.classList.remove('open');
  }
});

// ── Platform-specific tweaks ──────────────────────────────────────────────────
if (navigator.platform.startsWith('Mac')) document.body.classList.add('darwin');

// ── Initial state ─────────────────────────────────────────────────────────────
setStatus('', '');
updateNavButtons();
