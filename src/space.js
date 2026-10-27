'use strict';

// Disk space pages (once Xpace): what to scan (a search box for paths and
// folder names, the user's folders, recent scans, drives, or a dropped
// folder), scan progress, folder tree with a treemap, largest files and file
// types. The scan lives on the Rust side; folders are fetched one at a time as
// they are opened. Rows can be selected and moved to the Recycle Bin or, after
// a review, deleted for good. Its texts are the sp_ ones in space-i18n.js.

((gt) => {
const t = (key, vars) => gt(`sp_${key}`, vars);

const svg = (body) => `<svg class="i" viewBox="0 0 24 24">${body}</svg>`;
const ICONS = {
  drives: '<path d="M22 12H2"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/><path d="M6 16h.01M10 16h.01"/>',
  folders: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  files: '<path d="M3 3v18h18"/><rect x="7" y="12" width="3" height="6"/><rect x="12" y="8" width="3" height="10"/><rect x="17" y="5" width="3" height="13"/>',
  types: '<path d="M21.21 15.89A10 10 0 1 1 8 2.83"/><path d="M22 12A10 10 0 0 0 12 2v10z"/>',
  folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  file: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/>',
  link: '<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>',
  lock: '<rect x="4" y="11" width="16" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
  chevron: '<path d="m9 6 6 6-6 6"/>',
  reveal: '<path d="M15 3h6v6M10 14 21 3M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
  usb: '<path d="M10 2h4v6h-4z"/><rect x="7" y="8" width="10" height="14" rx="2"/>',
  network: '<rect x="16" y="16" width="6" height="6" rx="1"/><rect x="2" y="16" width="6" height="6" rx="1"/><rect x="9" y="2" width="6" height="6" rx="1"/><path d="M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3M12 12V8"/>',
  trash: '<path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14M10 11v5M14 11v5"/>',
  alert: '<path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/><path d="M12 9v4M12 17h.01"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
  home: '<path d="M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z"/>',
  monitor: '<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>',
  download: '<path d="M12 3v12M7 10l5 5 5-5M5 21h14"/>',
  image: '<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-5-5L5 21"/>',
  film: '<rect x="2" y="4" width="20" height="16" rx="2"/><path d="M7 4v16M17 4v16M2 9h5M2 15h5M17 9h5M17 15h5"/>',
  music: '<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>',
  package: '<path d="m21 8-9-5-9 5v8l9 5 9-5z"/><path d="m3 8 9 5 9-5M12 13v8"/>',
  clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
  x: '<path d="M18 6 6 18M6 6l12 12"/>',
};

/** Icons for the user's folders, by known folder id. */
const PLACE_ICONS = { profile: 'home', desktop: 'monitor', documents: 'file', downloads: 'download', pictures: 'image', videos: 'film', music: 'music', appdata: 'folder', programs: 'package' };

/** Treemap colours for folders, largest first; files share one muted tone. */
const PALETTE = ['#5173d6', '#3dd68c', '#c792ea', '#f5c451', '#4fd1e8', '#ff8a4c', '#8fa9ee', '#ff5468'];

let sub = 'drives';     // which of the four pages, by its Xpace name
let drives = null;
let places = null;      // the user's own folders, from known_folders
let recent = [];        // folders scanned lately, newest first
let query = '';         // what the search box holds
let hits = [];          // folders offered under the search box
let hitsFor = '';       // the text they answer
let hit = -1;           // the highlighted one
let searching = false;  // the folder index is still being built
let findSeq = 0;
let scanning = null;    // { path, files, folders, bytes, current } while a scan runs
let summary = null;     // the last finished scan
const listings = new Map(); // folder id -> listing from open_folder
const expanded = new Set([0]);
let selected = 0;       // folder shown in the treemap and the trail
let largest = null;
let types = null;
const picked = new Map(); // path -> size of each selected file or folder
let deleting = false;

// ---------- formatting ----------

const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const count = (n) => n.toLocaleString(locale());

/** Binary units like Explorer. */
function bytes(b) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
  let i = 0;
  while (b >= 1024 && i < units.length - 1) { b /= 1024; i++; }
  return `${b.toLocaleString(locale(), { maximumFractionDigits: i === 0 ? 0 : b < 10 ? 2 : 1 })} ${units[i]}`;
}

const pct = (part, whole) => (whole > 0 ? (part / whole) * 100 : 0);
const pctText = (p) => `${p.toLocaleString(locale(), { maximumFractionDigits: p < 10 ? 1 : 0 })}%`;
const date = (s) => (s ? new Date(s * 1000).toLocaleDateString(locale()) : '');

function elapsed(ms) {
  const s = ms / 1000;
  if (s < 60) return `${s.toLocaleString(locale(), { maximumFractionDigits: 1 })} s`;
  return `${Math.floor(s / 60)} min ${Math.round(s % 60)} s`;
}

const join = (dir, name) => (dir.endsWith('\\') ? dir + name : `${dir}\\${name}`);

function meter(p, cls = '') {
  return `<div class="meter ${cls}"><i style="width:${Math.min(100, p).toFixed(2)}%"></i></div>`;
}

/** Errors from Rust are message keys when we know them. */
function errText(e) {
  const key = String(e);
  if (I18N.en.ui[`sp_err_${key}`]) return t(`err_${key}`);
  return I18N.en.ui[`sp_${key}`] ? t(key) : key;
}

/** Fades a newly opened page in. */
function enter() {
  if (!matchMedia('(prefers-reduced-motion: reduce)').matches) {
    pageEl.animate([{ opacity: 0, transform: 'translateY(6px)' }, { opacity: 1, transform: 'none' }], { duration: 240, easing: 'cubic-bezier(.2, .7, .2, 1)' });
  }
}

// ---------- the page ----------

const SUBS = { drives: 'space', folders: 'spaceFolders', files: 'spaceFiles', types: 'spaceTypes' };
const onSpace = () => Object.values(SUBS).includes(current);
const subOf = (page) => Object.keys(SUBS).find((k) => SUBS[k] === page);

/** Redraws the page, or opens the right one when `sub` moved on. */
function render() {
  if (!onSpace()) return;
  if (SUBS[sub] !== current) go(SUBS[sub]);
  else window.render();
}

function tabs() {
  return `<div class="hw-tabs" role="tablist">${Object.entries(SUBS).map(([id, page]) =>
    `<button role="tab" aria-selected="${id === sub}" class="${id === sub ? 'on' : ''}" data-go="${page}"${id !== 'drives' && !summary ? ' disabled' : ''}>` +
    `${esc(gt(id === 'drives' ? 'spaceScan' : page))}</button>`).join('')}</div>`;
}

function renderPage() {
  sub = subOf(current);
  if (sub !== 'drives' && !summary && !scanning) { sub = 'drives'; return go('space'); }
  const keep = pageEl.scrollTop;
  if (scanning) $('title').textContent = t('scanningTitle', { path: scanning.path });
  else if (sub === 'drives') $('title').textContent = t('drivesTitle');
  const line = !scanning && summary && sub !== 'drives'
    ? `${summary.path} · ${t('scannedIn', { files: count(summary.files), folders: count(summary.folders), time: elapsed(summary.elapsedMs) })}` +
      (summary.freed ? ` · ${t('freed', { size: bytes(summary.freed) })}` : '')
    : '';
  pageEl.innerHTML = (scanning ? '' : tabs()) + (line ? `<div class="sp-sub">${esc(line)}</div>` : '') + '<div id="sp-body"></div>';
  renderSelbar();
  if (scanning) return renderScanning();
  ({ drives: renderDrives, folders: renderFolders, files: renderFiles, types: renderTypes })[sub]();
  if (sub === 'folders') pageEl.scrollTop = keep;
}

function actions() {
  const at = subOf(current);
  if (scanning || !summary || at === 'drives') return '';
  return `<button class="btn" data-act="rescan">${esc(t('rescan'))}</button><button class="btn" data-act="export">${esc(t('exportCsv'))}</button>`;
}

// ---------- selecting and deleting ----------

/** The checkbox and trash button of a row that can be deleted. */
function pickBox(path, size) {
  return `<input type="checkbox" class="pick" data-pick="${esc(path)}" data-size="${size}" title="${esc(t('pickTip'))}"${picked.has(path) ? ' checked' : ''}>`;
}
const trashButton = (path, size) => `<button class="reveal trash" data-trash="${esc(path)}" data-size="${size}" title="${esc(t('trashTip'))}">${svg(ICONS.trash)}</button>`;

function renderSelbar() {
  const bar = $('selbar');
  const show = picked.size > 0 && !scanning && (sub === 'folders' || sub === 'files');
  bar.hidden = !show;
  if (!show) return;
  const size = [...picked.values()].reduce((a, b) => a + b, 0);
  const off = deleting ? ' disabled' : '';
  bar.innerHTML = `<span class="sel-count">${svg(ICONS.trash)}<b>${esc(t('selected', { n: count(picked.size), size: bytes(size) }))}</b></span>
    <button class="btn ghost small" data-act="unpick"${off}>${esc(t('clearSel'))}</button>
    <button class="btn small" data-act="forever"${off}>${esc(t('forever'))}</button>
    <button class="btn primary small" data-act="bin"${off}>${svg(ICONS.trash)}${esc(t('toBin'))}</button>`;
}

/** Shows the dialog; resolves with the id of the button pressed, or null. */
function dialog(title, body, buttons) {
  const modal = $('space-modal');
  $('sp-modal-title').textContent = title;
  $('sp-modal-body').innerHTML = body;
  $('sp-modal-actions').innerHTML = buttons.map((b) => `<button class="btn ${b.cls || ''}" data-choice="${b.id}"${b.off ? ` disabled title="${esc(b.off)}"` : ''}>${esc(b.label)}</button>`).join('');
  modal.hidden = false;
  modal.querySelector('[data-choice]:not([disabled])')?.focus();
  return new Promise((resolve) => {
    const done = (value) => {
      modal.hidden = true;
      modal.onclick = null;
      document.removeEventListener('keydown', onKey, true);
      resolve(value);
    };
    const onKey = (e) => { if (e.key === 'Escape') { e.stopPropagation(); done(null); } };
    document.addEventListener('keydown', onKey, true);
    modal.onclick = (e) => {
      const choice = e.target.closest('[data-choice]');
      if (choice) done(choice.dataset.choice === 'cancel' ? null : choice.dataset.choice);
      else if (e.target === modal) done(null);
    };
  });
}

function pathList(paths) {
  const shown = paths.slice(0, 6).map((p) => `<li title="${esc(p)}"><bdi>${esc(p)}</bdi></li>`).join('');
  const more = paths.length > 6 ? `<li class="more">${esc(t('andMore', { n: count(paths.length - 6) }))}</li>` : '';
  return `<ul class="del-paths">${shown}${more}</ul>`;
}

const cancelButton = () => ({ id: 'cancel', label: t('cancel') });

async function moveToBin(paths, sizeOf = (p) => picked.get(p) || 0) {
  const size = paths.reduce((a, p) => a + sizeOf(p), 0);
  const choice = await dialog(paths.length === 1 ? t('binTitleOne') : t('binTitle', { n: count(paths.length) }),
    `${pathList(paths)}<p>${esc(t('binNote', { size: bytes(size) }))}</p>`,
    [cancelButton(), { id: 'go', label: t('toBin'), cls: 'primary' }]);
  if (!choice) return;
  await runDelete(() => invoke('recycle', { paths }), 'bin');
}

async function deleteForever(paths) {
  deleting = true;
  renderSelbar();
  toast(t('reviewing'));
  let preview;
  try {
    preview = await invoke('prepare_delete', { paths });
  } catch (e) {
    deleting = false;
    renderSelbar();
    toast(errText(e));
    return;
  }
  deleting = false;
  renderSelbar();
  const apps = preview.blockerError
    ? `<p class="del-warn">${esc(t('blockerError', { error: preview.blockerError }))}</p>`
    : preview.blockers.length
      ? `<ul class="del-apps">${preview.blockers.map((b) => `<li><b>${esc(b.name || '?')}</b><span>PID ${b.pid}</span><em class="${b.canTerminate ? '' : 'bad'}">${esc(t(b.canTerminate ? 'canClose' : 'protectedApp'))}</em></li>`).join('')}</ul>`
      : `<p class="note">${esc(t('noBlockers'))}</p>`;
  const body = `<p class="del-warn">${svg(ICONS.alert)}<span>${esc(t('foreverNote'))}</span></p>
    ${pathList(preview.paths)}
    <div class="del-facts"><span><b>${count(preview.files)}</b>${esc(t('factFiles'))}</span><span><b>${count(preview.folders)}</b>${esc(t('factFolders'))}</span><span><b>${bytes(preview.bytes)}</b>${esc(t('factSize'))}</span></div>
    <h4>${esc(t('blocking'))}</h4>${apps}`;
  const buttons = [cancelButton()];
  if (preview.blockers.length) {
    const closable = !preview.blockerError && preview.blockers.every((b) => b.canTerminate);
    buttons.push({ id: 'reboot', label: t('atRestart'), off: admin ? '' : t('restartNeedsAdmin') });
    if (closable) buttons.push({ id: 'terminate', label: t('closeAndDelete'), cls: 'danger' });
  }
  buttons.push({ id: 'delete', label: t('deleteNow'), cls: preview.blockers.length ? '' : 'danger' });
  const mode = await dialog(t('foreverTitle'), body, buttons);
  if (!mode) return;
  await runDelete(() => invoke('execute_delete', { token: preview.token, mode }), mode);
}

/** Runs a delete, then refreshes what is on screen and reports. */
async function runDelete(call, mode) {
  deleting = true;
  renderSelbar();
  let result;
  try {
    result = await call();
  } catch (e) {
    deleting = false;
    renderSelbar();
    toast(errText(e));
    return;
  }
  const freed = result.summary.freed - (summary?.freed || 0);
  summary = result.summary;
  for (const p of result.removed) picked.delete(p);
  await refreshAfterDelete();
  deleting = false;
  render();

  const o = result.outcome;
  if (mode === 'reboot' && o) {
    toast(t('scheduledDone', { n: count(o.scheduled) }));
  } else if (result.removed.length) {
    const key = (mode === 'bin' ? 'binDone' : 'foreverDone') + (result.removed.length === 1 ? 'One' : '');
    toast(t(key, { n: count(result.removed.length), size: bytes(freed) }));
  }
  const problems = [];
  if (result.error && !(mode === 'bin' && !result.kept.length)) problems.push(`<p class="del-warn">${esc(errText(result.error))}</p>`);
  if (o?.errors?.length) {
    problems.push(`<ul class="del-errors">${o.errors.map((e) => `<li><bdi>${esc(e.path)}</bdi><span>${esc(errText(e.error))}</span></li>`).join('')}</ul>`);
  } else if (result.kept.length && mode !== 'reboot') {
    problems.push(pathList(result.kept), `<p>${esc(t('keptNote'))}</p>`);
  }
  if (problems.length) await dialog(t('notDeleted'), problems.join(''), [{ id: 'cancel', label: t('close'), cls: 'primary' }]);
}

/** Folder listings, the largest files and the types are read again. */
async function refreshAfterDelete() {
  listings.clear();
  largest = null;
  types = null;
  for (const id of [...expanded]) {
    try {
      await loadFolder(id);
    } catch {
      expanded.delete(id);
    }
  }
  try {
    await loadFolder(selected);
  } catch {
    selected = 0;
  }
}

// ---------- drives ----------

function driveCards() {
  if (!drives) return `<div class="loading">${esc(t('loading'))}</div>`;
  return drives.map((d) => {
    const used = d.total - d.free;
    const p = pct(used, d.total);
    const icon = d.kind === 'removable' ? ICONS.usb : d.kind === 'network' ? ICONS.network : ICONS.drives;
    const name = d.label ? `${d.label} (${d.path.slice(0, 2)})` : `${t(d.kind)} (${d.path.slice(0, 2)})`;
    return `<button class="drive" data-scan="${esc(d.path)}">
      <div class="drive-top"><div class="ico">${svg(icon)}</div><div><b>${esc(name)}</b><small>${esc(t(d.kind))}${d.fileSystem ? ` · ${esc(d.fileSystem)}` : ''}</small></div></div>
      ${meter(p, p > 95 ? 'full' : p > 85 ? 'warn' : '')}
      <div class="use"><span>${esc(t('free', { free: bytes(d.free), total: bytes(d.total) }))}</span><b>${esc(t('used', { pct: Math.round(p) }))}</b></div>
    </button>`;
  }).join('');
}

/** "Dev" and "C:\Users\Vanildo" for C:\Users\Vanildo\Dev; a drive stays whole. */
function splitPath(path) {
  const trimmed = path.replace(/\\+$/, '');
  const at = trimmed.lastIndexOf('\\');
  if (at < 0 || /^[a-z]:$/i.test(trimmed)) return { name: path, parent: '' };
  return { name: trimmed.slice(at + 1), parent: trimmed.slice(0, at) || trimmed.slice(0, at + 1) };
}

function shortcuts() {
  const placeChips = (places || []).map((p) => `<button class="chip" data-scan="${esc(p.path)}" title="${esc(p.path)}">
    ${svg(ICONS[PLACE_ICONS[p.id]] || ICONS.folder)}<span>${esc(t(`place_${p.id}`))}</span></button>`).join('');
  const recentChips = recent.map((path) => {
    const { name, parent } = splitPath(path);
    return `<span class="chip recent"><button data-scan="${esc(path)}" title="${esc(path)}">${svg(ICONS.clock)}<span>${esc(name)}</span>${parent ? `<small>${esc(parent)}</small>` : ''}</button><button class="forget" data-forget="${esc(path)}" title="${esc(t('forgetRecent'))}" aria-label="${esc(t('forgetRecent'))}">${svg(ICONS.x)}</button></span>`;
  }).join('');
  return (placeChips ? `<section><h3 class="sect">${esc(t('quickAccess'))}</h3><div class="chips">${placeChips}</div></section>` : '')
    + (recentChips ? `<section><h3 class="sect">${esc(t('recent'))}</h3><div class="chips">${recentChips}</div></section>` : '');
}

function renderDrives() {
  $('sp-body').innerHTML = `<div class="finder">
      <div class="finder-bar">
        ${svg(ICONS.search)}
        <input id="q" type="text" spellcheck="false" autocomplete="off" role="combobox" aria-expanded="false" aria-controls="suggest" placeholder="${esc(t('searchPlaceholder'))}">
        <kbd>Ctrl L</kbd>
        <button class="btn" data-act="pick">${svg(ICONS.folder)}${esc(t('browse'))}</button>
      </div>
      <div class="suggest" id="suggest" role="listbox" hidden></div>
    </div>
    <div id="shortcuts">${shortcuts()}</div>
    <h3 class="sect">${esc(t('drivesHeading'))}</h3>
    <div class="drives" id="drive-cards">${driveCards()}</div>
    <p class="note">${esc(t('drivesNote'))}</p>`;
  const input = $('q');
  input.value = query;
  input.focus();
  if (query.trim()) suggest();
}

/** Redraws one part of the drives page, keeping the search box as it is. */
function refreshDrives(id, html) {
  if (sub !== 'drives' || scanning) return;
  if ($(id)) $(id).innerHTML = html();
  else render();
}

// ---------- the search box ----------

/** A typed path rather than a name: "C:...", a network path, "~", "%VAR%". */
const isPathText = (text) => /^\s*("?[a-z]:|\\\\|~|%)/i.test(text);

/** `name` with the part that matches what was typed marked. */
function marked(name) {
  const typed = query.trim();
  const words = isPathText(typed) ? [typed.slice(typed.lastIndexOf('\\') + 1)] : typed.split(/\s+/);
  const word = words.find((w) => w && name.toLowerCase().includes(w.toLowerCase()));
  if (!word) return esc(name);
  const at = name.toLowerCase().indexOf(word.toLowerCase());
  return `${esc(name.slice(0, at))}<mark>${esc(name.slice(at, at + word.length))}</mark>${esc(name.slice(at + word.length))}`;
}

function drawSuggest() {
  const box = $('suggest');
  const input = $('q');
  if (!box || !input) return;
  const open = document.activeElement === input && query.trim() !== '';
  box.hidden = !open;
  input.setAttribute('aria-expanded', String(open));
  if (!open) return;
  const rows = hits.map((h, i) => `<button class="sug${i === hit ? ' on' : ''}" data-scan="${esc(h.path)}" role="option" aria-selected="${i === hit}">
    ${svg(ICONS.folder)}<span><b>${marked(h.name)}</b><small>${esc(h.path)}</small></span></button>`).join('');
  const state = searching ? `<div class="sug-state"><i class="dot"></i>${esc(t('searchingFolders'))}</div>`
    : hits.length ? '' : `<div class="sug-state">${esc(t(isPathText(query) ? 'noSubfolders' : 'noMatches'))}</div>`;
  const keys = `<div class="sug-keys"><span><kbd>↑</kbd><kbd>↓</kbd> ${esc(t('keyChoose'))}</span><span><kbd>Enter</kbd> ${esc(t('keyScan'))}</span><span><kbd>Tab</kbd> ${esc(t('keyOpen'))}</span><span><kbd>Esc</kbd> ${esc(t('keyClear'))}</span></div>`;
  box.innerHTML = `<div class="sug-list">${rows}${state}</div>${keys}`;
  box.querySelector('.sug.on')?.scrollIntoView({ block: 'nearest' });
}

/** Asks for what matches the box; asks again while the index is still growing. */
async function suggest() {
  const seq = ++findSeq;
  const text = query;
  if (!text.trim()) {
    hits = [];
    hit = -1;
    searching = false;
    drawSuggest();
    return;
  }
  let found;
  try {
    found = await invoke('find_folders', { text });
  } catch {
    return;
  }
  if (seq !== findSeq) return;
  // Asking again for the same text (the index grew) keeps the chosen row.
  const before = text === hitsFor ? hits[hit]?.path : undefined;
  hits = found.hits;
  hitsFor = text;
  searching = found.searching;
  // A typed path scans as typed unless a row is chosen; a name search goes to the best match.
  const kept = hits.findIndex((h) => h.path === before);
  hit = kept >= 0 ? kept : isPathText(text) || !hits.length ? -1 : 0;
  drawSuggest();
  if (searching) setTimeout(() => { if (seq === findSeq) suggest(); }, 700);
}

function scanFromBox(path) {
  query = '';
  hits = [];
  hit = -1;
  // "C:\Dev\" scans as "C:\Dev"; a drive keeps its backslash.
  startScan(path.trim().replace(/([^:])\\+$/, '$1'));
}

function focusSearch() {
  if (scanning) return;
  if (current !== 'space') go('space');
  $('q')?.focus();
  $('q')?.select();
}

let typingTimer = 0;
pageEl.addEventListener('input', (e) => {
  if (e.target.id !== 'q') return;
  query = e.target.value;
  clearTimeout(typingTimer);
  typingTimer = setTimeout(suggest, isPathText(query) ? 90 : 200);
});

pageEl.addEventListener('keydown', (e) => {
  if (e.target.id !== 'q') return;
  const input = e.target;
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    if (!hits.length) return;
    e.preventDefault();
    const step = e.key === 'ArrowDown' ? 1 : -1;
    hit = hit < 0 ? (step > 0 ? 0 : hits.length - 1) : (hit + step + hits.length) % hits.length;
    drawSuggest();
  } else if (e.key === 'Enter') {
    e.preventDefault();
    const typed = query.trim();
    if (hit >= 0) scanFromBox(hits[hit].path);
    else if (isPathText(typed)) scanFromBox(typed);
  } else if (e.key === 'Tab' && hits.length && query.trim()) {
    // Opens the chosen folder in the box, to keep going down.
    e.preventDefault();
    query = `${hits[Math.max(hit, 0)].path}\\`;
    input.value = query;
    hit = -1;
    suggest();
  } else if (e.key === 'Escape') {
    if (query) {
      e.preventDefault();
      query = '';
      input.value = '';
      suggest();
    } else {
      input.blur();
    }
  }
});

pageEl.addEventListener('focusin', (e) => { if (e.target.id === 'q') drawSuggest(); });
// The box has lost focus only once the event is over.
pageEl.addEventListener('focusout', (e) => { if (e.target.id === 'q') setTimeout(drawSuggest); });
// Clicking a suggestion must not take the focus from the box first.
pageEl.addEventListener('mousedown', (e) => { if (e.target.closest('#suggest')) e.preventDefault(); });
pageEl.addEventListener('mousemove', (e) => {
  const row = e.target.closest('.sug');
  if (!row) return;
  const i = [...row.parentElement.querySelectorAll('.sug')].indexOf(row);
  if (i !== hit) {
    hit = i;
    $('suggest').querySelectorAll('.sug').forEach((el, n) => el.classList.toggle('on', n === i));
  }
});

document.addEventListener('keydown', (e) => {
  if (e.ctrlKey && !e.altKey && !e.shiftKey && e.key.toLowerCase() === 'l' && $('modal').hidden && $('space-modal').hidden) {
    e.preventDefault();
    focusSearch();
  }
});

// ---------- scanning ----------

function renderScanning() {
  const s = scanning;
  $('sp-body').innerHTML = `<div class="scanning">
    <div class="spinner"></div>
    <div class="counts">
      <div><b id="sc-files">${count(s.files)}</b><small>${esc(t('filesCount'))}</small></div>
      <div><b id="sc-folders">${count(s.folders)}</b><small>${esc(t('foldersCount'))}</small></div>
      <div><b id="sc-bytes">${bytes(s.bytes)}</b><small>${esc(t('sizeCount'))}</small></div>
    </div>
    <div class="current" id="sc-current"><bdi>${esc(s.current)}</bdi></div>
    <button class="btn" data-act="cancel">${esc(t('cancel'))}</button>
  </div>`;
}

function updateScanning() {
  if (!scanning || !$('sc-files')) return;
  $('sc-files').textContent = count(scanning.files);
  $('sc-folders').textContent = count(scanning.folders);
  $('sc-bytes').textContent = bytes(scanning.bytes);
  $('sc-current').firstChild.textContent = scanning.current;
}

async function startScan(path) {
  scanning = { path, files: 0, folders: 0, bytes: 0, current: path };
  render();
  try {
    const result = await invoke('start_scan', { path });
    if (scanning?.path !== path) return; // a newer scan took over
    summary = result;
    if (!result.cancelled) recent = [result.path, ...recent.filter((p) => p.toLowerCase() !== result.path.toLowerCase())].slice(0, 8);
    picked.clear();
    listings.clear();
    expanded.clear();
    expanded.add(0);
    selected = 0;
    largest = null;
    types = null;
    await loadFolder(0);
    sub = 'folders';
  } catch (e) {
    toast(t('failed', { error: errText(e) }));
  }
  scanning = null;
  render();
}

// ---------- folders ----------

async function loadFolder(id) {
  if (!listings.has(id)) listings.set(id, await invoke('open_folder', { id }));
  return listings.get(id);
}

function folderIcon(f) {
  if (f.link) return `<span class="k-link" title="${esc(t('linkTip'))}">${svg(ICONS.link)}</span>`;
  if (f.denied) return `<span class="k-denied" title="${esc(t('deniedTip'))}">${svg(ICONS.lock)}</span>`;
  return `<span class="k-folder">${svg(ICONS.folder)}</span>`;
}

function folderRow(f, parentSize, path, depth) {
  const open = expanded.has(f.id);
  const canOpen = !f.link && !f.denied && (f.files > 0 || f.folders > 0);
  const tag = f.link ? `<span class="tag">${esc(t('link'))}</span>` : f.denied ? `<span class="tag">${esc(t('denied'))}</span>` : '';
  const p = pct(f.size, parentSize);
  // The scanned folder itself, links and unreadable folders can't be deleted here.
  const deletable = f.id !== 0 && !f.link && !f.denied;
  return `<tr class="row${f.id === selected ? ' sel' : ''}${picked.has(path) ? ' picked' : ''}" data-id="${f.id}" data-path="${esc(path)}">
    <td><div class="name" style="padding-left:${depth * 18}px">
      ${deletable ? pickBox(path, f.size) : '<span class="pick-space"></span>'}
      <button class="tw${open ? ' open' : ''}${canOpen ? '' : ' none'}" data-tw="${f.id}">${svg(ICONS.chevron)}</button>
      ${folderIcon(f)}<span class="label" title="${esc(path)}">${esc(f.name)}</span>${tag}
      <button class="reveal" data-open="${esc(path)}" title="${esc(t('openFolder'))}">${svg(ICONS.reveal)}</button>
      ${deletable ? trashButton(path, f.size) : ''}
    </div></td>
    <td>${bytes(f.size)}</td>
    <td class="bar">${meter(p)}</td>
    <td class="muted">${pctText(p)}</td>
    <td class="muted">${count(f.files)}</td>
    <td class="muted">${count(f.folders)}</td>
    <td class="muted">${date(f.modified)}</td>
  </tr>`;
}

function fileRow(file, parentSize, path, depth) {
  const p = pct(file.size, parentSize);
  return `<tr class="row${picked.has(path) ? ' picked' : ''}" data-path="${esc(path)}">
    <td><div class="name" style="padding-left:${depth * 18}px">
      ${pickBox(path, file.size)}<span class="tw none"></span><span class="k-file">${svg(ICONS.file)}</span><span class="label" title="${esc(path)}">${esc(file.name)}</span>
      <button class="reveal" data-reveal="${esc(path)}" title="${esc(t('reveal'))}">${svg(ICONS.reveal)}</button>
      ${trashButton(path, file.size)}
    </div></td>
    <td>${bytes(file.size)}</td>
    <td class="bar">${meter(p)}</td>
    <td class="muted">${pctText(p)}</td>
    <td></td><td></td>
    <td class="muted">${date(file.modified)}</td>
  </tr>`;
}

/** Rows of an expanded folder, its open subfolders nested below each one. */
function childRows(id, depth) {
  const l = listings.get(id);
  if (!l) return '';
  let out = '';
  for (const f of l.folders) {
    const path = join(l.path, f.name);
    out += folderRow(f, l.folder.size, path, depth);
    if (expanded.has(f.id)) out += childRows(f.id, depth + 1);
  }
  for (const file of l.files) out += fileRow(file, l.folder.size, join(l.path, file.name), depth);
  if (l.moreFiles) {
    out += `<tr><td class="muted"><div class="name" style="padding-left:${depth * 18 + 24}px">${esc(t('moreFiles', { n: count(l.moreFiles) }))}</div></td>
      <td>${bytes(l.moreBytes)}</td><td></td><td class="muted">${pctText(pct(l.moreBytes, l.folder.size))}</td><td></td><td></td><td></td></tr>`;
  }
  return out;
}

function renderFolders() {
  const root = listings.get(0);
  const sel = listings.get(selected) || root;
  if (!root) {
    $('sp-body').innerHTML = `<div class="loading">${esc(t('loading'))}</div>`;
    return;
  }
  const notes = [];
  if (summary.cancelled) notes.push(t('cancelled'));
  if (summary.errors) notes.push(t('unreadable', { n: count(summary.errors) }));
  $('sp-body').innerHTML = `${notes.map((n) => `<div class="warn-note">${esc(n)}</div>`).join('')}
    <div class="trail" id="trail"></div>
    <div class="treemap" id="treemap"></div>
    <table class="grid">
      <colgroup><col><col style="width:96px"><col style="width:100px"><col style="width:96px"><col style="width:90px"><col style="width:76px"><col style="width:96px"></colgroup>
      <thead><tr><th>${esc(t('name'))}</th><th>${esc(t('size'))}</th><th></th><th>${esc(t('share'))}</th><th>${esc(t('filesCol'))}</th><th>${esc(t('foldersCol'))}</th><th>${esc(t('modified'))}</th></tr></thead>
      <tbody>${folderRow(root.folder, root.folder.size, root.path, 0)}${expanded.has(0) ? childRows(0, 1) : ''}</tbody>
    </table>`;
  paintSelection(sel);
}

/** The trail, the treemap and the highlighted row for the selected folder. */
function paintSelection(sel) {
  $('trail').innerHTML = sel.trail.map((f, i) => `${i ? '<span>›</span>' : ''}<button data-select="${f.id}">${esc(f.name)}</button>`).join('');
  for (const row of pageEl.querySelectorAll('tr.sel')) row.classList.remove('sel');
  pageEl.querySelector(`tr[data-id="${sel.folder.id}"]`)?.classList.add('sel');
  drawTreemap(sel);
}

/** Squarified treemap (Bruls et al.): rows of blocks kept close to square. */
function squarify(items, x, y, w, h) {
  const out = [];
  const worst = (row, side) => {
    const sum = row.reduce((a, r) => a + r.area, 0);
    const max = row[0].area, min = row[row.length - 1].area;
    return Math.max((side * side * max) / (sum * sum), (sum * sum) / (side * side * min));
  };
  const place = (row) => {
    const sum = row.reduce((a, r) => a + r.area, 0);
    if (w >= h) {
      const cw = sum / h;
      let cy = y;
      for (const r of row) { out.push({ ...r, x, y: cy, w: cw, h: r.area / cw }); cy += r.area / cw; }
      x += cw; w -= cw;
    } else {
      const rh = sum / w;
      let cx = x;
      for (const r of row) { out.push({ ...r, x: cx, y, w: r.area / rh, h: rh }); cx += r.area / rh; }
      y += rh; h -= rh;
    }
  };
  let row = [];
  for (const item of items) {
    const side = Math.min(w, h);
    if (!row.length || worst([...row, item], side) <= worst(row, side)) {
      row.push(item);
    } else {
      place(row);
      row = [item];
    }
  }
  if (row.length) place(row);
  return out;
}

function drawTreemap(l) {
  const box = $('treemap');
  const W = box.clientWidth, H = box.clientHeight;
  const total = l.folder.size;
  if (!total || !W) {
    box.innerHTML = `<div class="tm-empty">${esc(t('emptyFolder'))}</div>`;
    return;
  }
  let items = [
    ...l.folders.filter((f) => f.size > 0).map((f, i) => ({ kind: 'folder', id: f.id, name: f.name, size: f.size, color: PALETTE[i % PALETTE.length] })),
    ...l.files.filter((f) => f.size > 0).map((f) => ({ kind: 'file', name: f.name, size: f.size, path: join(l.path, f.name) })),
  ].sort((a, b) => b.size - a.size);
  // Blocks too small to read are merged into one.
  const minSize = total * 0.004;
  const small = items.filter((i) => i.size < minSize);
  items = items.filter((i) => i.size >= minSize);
  const restSize = small.reduce((a, i) => a + i.size, 0) + l.moreBytes;
  if (restSize > 0) items.push({ kind: 'rest', name: `+${count(small.length + l.moreFiles)}`, size: restSize });
  const scale = (W * H) / items.reduce((a, i) => a + i.size, 0);
  const blocks = squarify(items.map((i) => ({ ...i, area: i.size * scale })), 0, 0, W, H);
  box.innerHTML = blocks.map((b) => {
    const style = `left:${b.x}px;top:${b.y}px;width:${b.w}px;height:${b.h}px;${b.kind === 'folder' ? `background-color:${b.color}` : b.kind === 'file' ? 'background-color:#2a3958' : ''}`;
    const label = b.w > 46 && b.h > 30 ? `<b>${esc(b.name)}</b><small>${bytes(b.size)} · ${pctText(pct(b.size, total))}</small>` : '';
    const data = b.kind === 'folder' ? ` data-select="${b.id}"` : b.kind === 'file' ? ` data-path="${esc(b.path)}"` : '';
    return `<div class="tm ${b.kind}" style="${style}"${data} title="${esc(`${b.name} · ${bytes(b.size)}`)}">${label}</div>`;
  }).join('');
}

/** Opens a folder in the treemap and unfolds the tree down to it. */
async function selectFolder(id) {
  const l = await loadFolder(id);
  // Every folder above it must be open for its row to show.
  let unfolded = false;
  for (const f of l.trail.slice(0, -1)) {
    if (!expanded.has(f.id)) {
      expanded.add(f.id);
      unfolded = true;
    }
    await loadFolder(f.id);
  }
  selected = id;
  if (sub !== 'folders') return;
  // Redraw only when rows appear, so a double-click still lands on its row.
  if (unfolded || !$('trail')) renderFolders();
  else paintSelection(l);
  pageEl.querySelector(`tr[data-id="${id}"]`)?.scrollIntoView({ block: 'nearest' });
}

async function toggleFolder(id) {
  if (expanded.has(id)) {
    expanded.delete(id);
  } else {
    await loadFolder(id);
    expanded.add(id);
  }
  renderFolders();
}

// ---------- largest files and file types ----------

async function renderFiles() {
  if (!largest) {
    $('sp-body').innerHTML = `<div class="loading">${esc(t('loading'))}</div>`;
    largest = await invoke('largest_files');
    if (sub !== 'files') return;
  }
  const rows = largest.map((f) => {
    const name = f.path.slice(f.path.lastIndexOf('\\') + 1);
    const dir = f.path.slice(0, f.path.lastIndexOf('\\'));
    return `<tr class="row${picked.has(f.path) ? ' picked' : ''}" data-path="${esc(f.path)}">
      <td><div class="name">${pickBox(f.path, f.size)}<span class="k-file">${svg(ICONS.file)}</span><span class="label" title="${esc(f.path)}">${esc(name)}</span>
        <button class="reveal" data-reveal="${esc(f.path)}" title="${esc(t('reveal'))}">${svg(ICONS.reveal)}</button>${trashButton(f.path, f.size)}</div></td>
      <td>${bytes(f.size)}</td>
      <td class="muted path-cell" title="${esc(dir)}"><bdi>${esc(dir)}</bdi></td>
      <td class="muted">${date(f.modified)}</td>
    </tr>`;
  }).join('');
  $('sp-body').innerHTML = `<table class="grid">
    <colgroup><col style="width:34%"><col style="width:96px"><col><col style="width:96px"></colgroup>
    <thead><tr><th>${esc(t('name'))}</th><th>${esc(t('size'))}</th><th class="left">${esc(t('path'))}</th><th>${esc(t('modified'))}</th></tr></thead>
    <tbody>${rows}</tbody></table>`;
}

async function renderTypes() {
  if (!types) {
    $('sp-body').innerHTML = `<div class="loading">${esc(t('loading'))}</div>`;
    types = await invoke('file_types');
    if (sub !== 'types') return;
  }
  const total = summary.size;
  const top = types[0]?.size || 1;
  const rows = types.slice(0, 300).map((x) => `<tr class="row">
      <td><div class="name"><span class="k-file">${svg(ICONS.file)}</span><span class="label">${esc(x.ext || t('noExt'))}</span></div></td>
      <td>${bytes(x.size)}</td>
      <td class="bar">${meter(pct(x.size, top))}</td>
      <td class="muted">${pctText(pct(x.size, total))}</td>
      <td class="muted">${count(x.files)}</td>
    </tr>`).join('');
  $('sp-body').innerHTML = `${summary.freed ? `<div class="warn-note">${esc(t('typesAfterDelete'))}</div>` : ''}<div class="stats">
      <div class="stat"><small>${esc(t('sizeCount'))}</small><b>${bytes(summary.size)}</b></div>
      <div class="stat"><small>${esc(t('filesCount'))}</small><b>${count(summary.files)}</b></div>
      <div class="stat"><small>${esc(t('foldersCount'))}</small><b>${count(summary.folders)}</b></div>
      <div class="stat"><small>${esc(t('types'))}</small><b>${count(types.length)}</b></div>
    </div>
    <table class="grid">
    <colgroup><col><col style="width:96px"><col style="width:180px"><col style="width:90px"><col style="width:96px"></colgroup>
    <thead><tr><th>${esc(t('type'))}</th><th>${esc(t('size'))}</th><th></th><th>${esc(t('ofTotal'))}</th><th>${esc(t('filesCol'))}</th></tr></thead>
    <tbody>${rows}</tbody></table>`;
}

// ---------- CSV export ----------

function csv(rows) {
  const sep = language === 'en' ? ',' : ';';
  const cell = (v) => (typeof v === 'number' ? String(v) : `"${String(v ?? '').replace(/"/g, '""')}"`);
  return rows.map((r) => r.map(cell).join(sep)).join('\r\n') + '\r\n';
}

async function exportCsv() {
  let rows;
  if (sub === 'files') {
    rows = [[t('path'), t('size'), t('modified')], ...largest.map((f) => [f.path, f.size, date(f.modified)])];
  } else if (sub === 'types') {
    rows = [[t('type'), t('size'), t('filesCol')], ...types.map((x) => [x.ext || t('noExt'), x.size, x.files])];
  } else {
    const l = listings.get(selected);
    rows = [[t('path'), t('size'), t('filesCol'), t('foldersCol'), t('modified')],
      ...l.folders.map((f) => [join(l.path, f.name), f.size, f.files, f.folders, date(f.modified)]),
      ...l.files.map((f) => [join(l.path, f.name), f.size, 1, 0, date(f.modified)])];
  }
  const stamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-');
  try {
    const path = await invoke('save_export', { name: `xpace-${sub}-${stamp}`, csv: csv(rows) });
    toast(t('saved', { path }));
  } catch (e) {
    toast(t('failed', { error: e }));
  }
}

// ---------- events ----------

async function reveal(path, open) {
  try {
    await invoke('reveal', { path, open });
  } catch (e) {
    toast(errText(e));
  }
}

document.addEventListener('click', async (e) => {
  const el = e.target.closest('[data-act],[data-scan],[data-forget],[data-tw],[data-select],[data-reveal],[data-open],tr[data-id]');
  if (!el || el.disabled || !onSpace()) return;
  const d = el.dataset;
  if (d.scan) {
    scanFromBox(d.scan);
  } else if (d.forget) {
    recent = await invoke('forget_recent', { path: d.forget }).catch(() => recent.filter((p) => p !== d.forget));
    refreshDrives('shortcuts', shortcuts);
  } else if (d.tw) {
    toggleFolder(Number(d.tw));
  } else if (d.select) {
    selectFolder(Number(d.select));
  } else if (d.reveal) {
    reveal(d.reveal, false);
  } else if (d.open) {
    reveal(d.open, true);
  } else if (d.id) {
    selectFolder(Number(d.id));
  } else if (d.act === 'pick') {
    const start = isPathText(query) ? query.trim() : null;
    const path = await invoke('pick_scan_folder', { start }).catch((err) => { toast(errText(err)); return null; });
    if (path) scanFromBox(path);
  } else if (d.act === 'cancel') {
    invoke('cancel_scan');
  } else if (d.act === 'rescan') {
    startScan(summary.path);
  } else if (d.act === 'export') {
    exportCsv();
  } else if (d.act === 'unpick') {
    picked.clear();
    render();
  } else if (d.act === 'bin' && !deleting) {
    moveToBin([...picked.keys()]);
  } else if (d.act === 'forever' && !deleting) {
    deleteForever([...picked.keys()]);
  }
});

// Checkboxes and trash buttons sit inside rows that open folders on click:
// handle them first and keep the row from reacting.
document.addEventListener('click', (e) => {
  const box = e.target.closest('[data-pick]');
  const trash = e.target.closest('[data-trash]');
  if ((!box && !trash) || !onSpace()) return;
  e.stopPropagation();
  if (box) {
    // The box is already ticked or unticked when the click arrives.
    if (box.checked) picked.set(box.dataset.pick, Number(box.dataset.size));
    else picked.delete(box.dataset.pick);
    box.closest('tr')?.classList.toggle('picked', box.checked);
    renderSelbar();
  } else if (!deleting) {
    moveToBin([trash.dataset.trash], () => Number(trash.dataset.size));
  }
}, true);

document.addEventListener('dblclick', (e) => {
  const row = e.target.closest('tr[data-path]');
  if (!onSpace() || !row || e.target.closest('button') || e.target.closest('[data-pick]')) return;
  reveal(row.dataset.path, row.dataset.id !== undefined);
});

// A folder dragged in from Explorer is scanned.
const dropEl = $('drop');
listen('tauri://drag-enter', () => { if (onSpace() && !scanning && $('modal').hidden && $('space-modal').hidden) dropEl.hidden = false; });
listen('tauri://drag-leave', () => { dropEl.hidden = true; });
listen('tauri://drag-drop', ({ payload }) => {
  const shown = !dropEl.hidden;
  dropEl.hidden = true;
  const path = payload?.paths?.[0];
  if (shown && path && !scanning) scanFromBox(path);
});

listen('progress', ({ payload }) => {
  if (!scanning) return;
  Object.assign(scanning, payload);
  updateScanning();
});

let resizeTimer = 0;
window.addEventListener('resize', () => {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => { if (onSpace() && sub === 'folders' && !scanning && listings.has(selected)) drawTreemap(listings.get(selected)); }, 120);
});

const page = { render: renderPage, actions, nav: 'space', cls: 'space', show: () => {
  invoke('list_drives').then((list) => { drives = list; refreshDrives('drive-cards', driveCards); });
} };
for (const id of Object.values(SUBS)) PAGES[id] = page;

invoke('get_settings').then((settings) => { recent = settings.recent || []; }, () => {});
invoke('known_folders').then((list) => { places = list; refreshDrives('shortcuts', shortcuts); });
})(t);
