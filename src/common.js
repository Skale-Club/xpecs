'use strict';

// What every page shares: Tauri calls, formatting, the toast, the confirm
// box, icons and the page registry. app.js owns navigation; each page file
// (care.js, drivers.js, hardware.js, force-delete.js) adds its pages to PAGES.

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
const pageEl = $('page');

/** id -> { render(), actions?() for the header, show?() on each visit }. */
const PAGES = {};
let current = 'overview';
let admin = false;

const MB = 1024 ** 2, GB = 1024 ** 3;

const svg = (body, cls = 'i') => `<svg class="${cls}" viewBox="0 0 24 24">${body}</svg>`;
const ICONS = {
  forceDelete: '<path d="M4 7h16M9 7V4h6v3M6 7l1 14h10l1-14M10 11v6M14 11v6"/><path d="m18 2 3 3m0-3-3 3"/>',
  overview: '<path d="M12 3a9 9 0 1 0 9 9"/><path d="M12 7v5l3 2"/><path d="M16 3h5v5"/>',
  cleaner: '<path d="m15 4 5 5"/><path d="M13.5 5.5 4 15l5 5 9.5-9.5"/><path d="M7 18l-3 3"/>',
  startup: '<path d="M12 2v10"/><path d="M18.4 6.6a9 9 0 1 1-12.77.04"/>',
  apps: '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><path d="M14 17.5h7M17.5 14v7"/>',
  optimize: '<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>',
  space: '<path d="M21.21 15.89A10 10 0 1 1 8 2.83"/><path d="M22 12A10 10 0 0 0 12 2v10z"/>',
  drivers: '<path d="M9 2v4M15 2v4M7 6h10v5a5 5 0 0 1-10 0z"/><path d="M12 16v6"/>',
  ok: '<path d="M5 12.5 10 17l9-10"/>',
  warn: '<path d="M12 5.5v8.5M12 18.5v.01"/>',
  bad: '<path d="M7 7l10 10M17 7 7 17"/>',
  repair: '<path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>',
  restore: '<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l3 2"/>',
  defrag: '<rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/>',
  visual: '<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
  gamemode: '<rect x="2" y="7" width="20" height="10" rx="5"/><path d="M7 10v4M5 12h4M15.5 11.5h.01M18 12.5h.01"/>',
  storageSense: '<path d="M22 12H2"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/>',
  update: '<path d="M21 12a9 9 0 0 1-15.5 6.3L3 16"/><path d="M3 12a9 9 0 0 1 15.5-6.3L21 8"/><path d="M21 3v5h-5M3 21v-5h5"/>',
  taskmgr: '<path d="M22 12h-4l-3 9L9 3l-3 9H2"/>',
  cleanmgr: '<path d="M3 6h18"/><path d="M8 6V4h8v2"/><path d="m19 6-1 14H6L5 6"/>',
  devmgmt: '<rect x="2" y="4" width="20" height="13" rx="2"/><path d="M8 21h8M12 17v4"/><path d="m9 10 2 2 4-4"/>',
  summary: '<rect x="3" y="3" width="7" height="9" rx="1.5"/><rect x="14" y="3" width="7" height="5" rx="1.5"/><rect x="14" y="12" width="7" height="9" rx="1.5"/><rect x="3" y="16" width="7" height="5" rx="1.5"/>',
  cpu: '<rect x="5" y="5" width="14" height="14" rx="2"/><rect x="9" y="9" width="6" height="6" rx="1"/><path d="M9 2v3M15 2v3M9 19v3M15 19v3M2 9h3M2 15h3M19 9h3M19 15h3"/>',
  board: '<rect x="3" y="3" width="18" height="18" rx="2"/><rect x="7" y="7" width="5" height="5" rx="1"/><path d="M15 7h2M15 10h2M7 16h10"/>',
  memory: '<path d="M3 7h18v8H3z"/><path d="M7 10v2M11 10v2M15 10v2"/><path d="M6 15v3M10 15v3M14 15v3M18 15v3"/>',
  gpu: '<rect x="2" y="6" width="20" height="12" rx="2"/><circle cx="9" cy="12" r="3"/><path d="M15 10h4M15 14h4"/>',
  storage: '<path d="M22 12H2"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/><path d="M6 16h.01M10 16h.01"/>',
  network: '<rect x="16" y="16" width="6" height="6" rx="1"/><rect x="2" y="16" width="6" height="6" rx="1"/><rect x="9" y="2" width="6" height="6" rx="1"/><path d="M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3M12 12V8"/>',
  os: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18M7 6.5h.01M10 6.5h.01"/>',
  sensors: '<path d="M22 12h-4l-3 9L9 3l-3 9H2"/>',
  bench: '<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>',
  export: '<path d="M12 3v12M7 10l5 5 5-5M5 21h14"/>',
};

// ---------- formatting ----------

const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const num = (v, d = 0) => v.toLocaleString(locale(), { maximumFractionDigits: d });
const fixed = (v, d = 0) => v.toLocaleString(locale(), { minimumFractionDigits: d, maximumFractionDigits: d });

/** Binary units like Explorer; whole numbers stay whole ("16 GB"). */
function bytes(b) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let i = 0;
  while (b >= 1024 && i < units.length - 1) { b /= 1024; i++; }
  return `${num(b, i >= 3 ? 1 : 0)} ${units[i]}`;
}

/** t() with {n}, using the "…One" text when n is 1 and there is one. */
const tn = (key, n, vars = {}) => t(n === 1 && I18N.en.ui[`${key}One`] ? `${key}One` : key, { n, ...vars });

// ---------- feedback ----------

function toast(text) {
  const el = $('toast');
  el.textContent = text;
  el.classList.add('show');
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => el.classList.remove('show'), 2800);
}

function confirmBox(title, text, okLabel, danger) {
  return new Promise((resolve) => {
    const modal = $('modal');
    $('modal-title').textContent = title;
    $('modal-text').textContent = text;
    $('modal-ok').textContent = okLabel;
    $('modal-ok').className = `btn ${danger ? 'danger' : 'primary'}`;
    $('modal-cancel').textContent = t('cancel');
    let settled = false;
    const previousFocus = document.activeElement;
    const done = (answer) => {
      if (settled) return;
      settled = true;
      modal.hidden = true;
      document.removeEventListener('keydown', onKey);
      if (previousFocus?.isConnected) previousFocus.focus();
      resolve(answer);
    };
    const onKey = (e) => {
      if (e.key === 'Escape') { e.preventDefault(); done(false); }
      if (e.key === 'Tab') {
        const first = $('modal-cancel'), last = $('modal-ok');
        if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
        else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
      }
    };
    $('modal-ok').onclick = () => done(true);
    $('modal-cancel').onclick = () => done(false);
    document.addEventListener('keydown', onKey);
    modal.hidden = false;
    $('modal-ok').focus();
  });
}

/** Runs `fn`; a failure becomes a toast and undefined. */
async function attempt(fn) {
  try {
    return await fn();
  } catch (e) {
    toast(String(e) === 'cancelled' ? t('cancelled') : t('failed', { error: e }));
    return undefined;
  }
}

/** The "restart as administrator" banner, for pages with actions that need it. */
const adminBanner = (text) => (admin ? '' :
  `<div class="banner"><span>${esc(text)}</span><button class="btn small" data-action="admin">${esc(t('restartAdmin'))}</button></div>`);
