'use strict';

// PC care pages (once Xcare): health overview, cleaner, startup programs,
// installed apps and the optimize page. Everything that changes the system
// goes through a Rust command and, when it deletes or uninstalls, a
// confirmation first.

const TOOLS = ['repair', 'restore', 'defrag', 'visual', 'gamemode', 'storage', 'update', 'taskmgr', 'cleanmgr', 'devmgmt'];
const ADMIN_TOOLS = ['repair', 'restore'];
const POWER_SAVER = 'a1841308-3541-4fab-bc81-f71556f20b4a';
const ULTIMATE_NAME = /ultimate|m[aá]xim|definitiv/i;

let status = null;
let junk = null;          // categories from the last scan
let startup = null;
let apps = null;
let plans = null;
let checkedAt = null;
let busy = null;          // 'check', 'scan' or 'clean'
let lastClean = null;
let selected = null;      // Set of category ids, filled from defaults after the first scan
let appsQuery = '';
let appsSort = 'size';
let progress = null;      // last "scan-progress" / "clean-progress" event, with its kind

function since(seconds) {
  const d = Math.floor(seconds / 86400), h = Math.floor((seconds % 86400) / 3600), m = Math.floor((seconds % 3600) / 60);
  return d ? `${d}${t('d')} ${h}${t('h')}` : `${h}${t('h')} ${m}${t('mins')}`;
}

const cleanable = () => (junk || []).filter((c) => !c.locked && selected?.has(c.id)).reduce((s, c) => s + c.bytes, 0);
const defaultJunk = () => (junk || []).filter((c) => c.default && !c.locked).reduce((s, c) => s + c.bytes, 0);
const lockedJunk = () => (junk || []).filter((c) => c.locked).reduce((s, c) => s + c.bytes, 0);

function planName(p) {
  const known = I18N.en.plans[p.guid];
  if (known) return pair('plans', p.guid)[0];
  if (ULTIMATE_NAME.test(p.name)) return pair('plans', 'ultimate')[0];
  return p.name;
}

function planDescription(p) {
  if (I18N.en.plans[p.guid]) return pair('plans', p.guid)[1];
  return pair('plans', ULTIMATE_NAME.test(p.name) ? 'ultimate' : 'custom')[1];
}

// ---------- data ----------

// Real icons of startup programs, installed programs and cleaner categories,
// by id; they arrive after the lists so those show at once.
const icons = { startup: {}, apps: {}, cleaner: {} };

function loadIcons(kind, repaint) {
  invoke('list_icons', { kind }).then((map) => { icons[kind] = map; repaint(); }, () => {});
}

/** The item's own icon, or its first letter on a tile when it has none. */
function itemIcon(kind, id, name) {
  const url = icons[kind][id];
  if (url) return `<img class="app-icon" src="${esc(url)}" alt="" draggable="false">`;
  const letter = [...String(name || '?').trim()][0] || '?';
  return `<span class="app-icon none">${esc(letter.toUpperCase())}</span>`;
}

const repaintPage = (page) => () => { if (current === page && !busy) render(); };

async function refreshStatus() {
  status = await invoke('get_status');
  admin = status.admin;
  paintAdmin();
}

async function scanJunk() {
  progress = null;
  junk = await invoke('scan_junk');
  progress = null;
  if (!selected) selected = new Set(junk.filter((c) => c.default && !c.locked).map((c) => c.id));
}

async function checkAll() {
  busy = 'check';
  render();
  await Promise.all([
    refreshStatus(),
    scanJunk(),
    invoke('list_startup').then((s) => { startup = s; loadIcons('startup', repaintPage('startup')); }),
    invoke('list_power_plans').then((p) => { plans = p; }),
  ]);
  checkedAt = new Date();
  busy = null;
  render();
}

// ---------- overview ----------

function findings() {
  const items = [];
  const add = (level, penalty, title, text, action) => items.push({ level, penalty, title, text, action });

  const junkBytes = defaultJunk();
  add(junkBytes >= 5 * GB ? 'bad' : junkBytes >= GB ? 'warn' : 'ok', junkBytes >= 5 * GB ? 25 : junkBytes >= GB ? 12 : 0,
    t('junk'), junkBytes >= 100 * MB ? t('junkFound', { size: bytes(junkBytes) }) : t('junkOk'),
    junkBytes >= 100 * MB ? ['clean', 'go:cleaner'] : null);

  const on = (startup || []).filter((s) => s.enabled).length;
  add(on > 12 ? 'warn' : on > 8 ? 'warn' : 'ok', on > 12 ? 15 : on > 8 ? 8 : 0,
    t('startupItems'), on > 8 ? t('startupMany', { n: on }) : t('startupOk', { n: on }), on > 8 ? ['review', 'go:startup'] : null);

  if (status.total) {
    // Percent alone misleads on big drives: 11% of 2 TB is still 200 GB.
    const pct = status.free / status.total;
    const low = status.free < 10 * GB || pct < 0.05 ? 'bad' : status.free < 30 * GB || pct < 0.1 ? 'warn' : 'ok';
    add(low, low === 'bad' ? 20 : low === 'warn' ? 8 : 0,
      t('space', { drive: status.drive }),
      t(low === 'ok' ? 'spaceOk' : 'spaceLow', { free: bytes(status.free), total: bytes(status.total) }),
      low === 'ok' ? null : ['clean', 'go:cleaner']);
  }

  const days = status.uptime / 86400;
  add(days > 7 ? 'warn' : 'ok', days > 14 ? 10 : days > 7 ? 5 : 0,
    t('uptime'), t(days > 7 ? 'uptimeLong' : 'uptimeOk', { time: since(status.uptime) }), null);

  const active = (plans || []).find((p) => p.active);
  if (active) {
    const saver = active.guid === POWER_SAVER;
    add(saver ? 'warn' : 'ok', saver ? 8 : 0, t('power'),
      saver ? t('powerSaver') : t('powerOk', { name: planName(active) }), saver ? ['change', 'go:optimize'] : null);
  }

  const sick = status.unhealthyDisks || [];
  add(sick.length ? 'bad' : 'ok', sick.length ? 30 : 0, t('drives'),
    sick.length ? t('drivesBad', { names: sick.join(', ') }) : t('drivesOk'), null);

  add(status.memoryLoad > 90 ? 'warn' : 'ok', status.memoryLoad > 90 ? 5 : 0, t('memory'),
    t(status.memoryLoad > 90 ? 'memoryHigh' : 'memoryOk', { load: status.memoryLoad }), null);

  const problems = window.XpecsDrivers?.problemCount();
  if (problems) add('warn', 6, t('drvTitle'), tn('drvProblemsFound', problems), ['review', 'go:drivers']);

  if (!status.admin && lockedJunk() >= 200 * MB) {
    add('warn', 0, t('adminMore'), t('adminMoreText', { size: bytes(lockedJunk()) }), ['restartAdmin', 'admin']);
  }
  // Problems first.
  const order = { bad: 0, warn: 1, ok: 2 };
  return items.sort((a, b) => order[a.level] - order[b.level]);
}

// ---------- progress ----------

function progressInner() {
  const p = progress;
  const name = p.category ? pair('cats', p.category)[0] : '';
  let fraction, detail;
  if (p.kind === 'clean') {
    fraction = p.bytesTotal ? p.bytesDone / p.bytesTotal : p.total ? p.done / p.total : 1;
    detail = t('progressFiles', { done: num(p.done), total: num(p.total), freed: bytes(p.bytesDone), size: bytes(p.bytesTotal) });
  } else {
    fraction = p.total ? p.done / p.total : 0;
    detail = t('progressCats', { done: Math.min(p.done + 1, p.total), total: p.total });
  }
  const percent = Math.round(Math.min(1, fraction) * 100);
  return `<div class="row-between"><b>${esc(t(p.kind === 'clean' ? 'progressClean' : 'progressScan', { name }))}</b>` +
    `<span class="pct">${percent}%</span></div><div class="pbar"><i style="width:${percent}%"></i></div>` +
    `<div class="note">${esc(detail)}</div>`;
}

const progressCard = () => (busy && progress ? `<div class="card progress" id="progress">${progressInner()}</div>` : '');

/** Updates the bar in place, so a fast stream of events doesn't rebuild the page. */
function paintProgress() {
  const el = $('progress');
  if (el) el.innerHTML = progressInner();
  else if (busy) render();
}

listen('scan-progress', ({ payload }) => { progress = { kind: 'scan', ...payload }; paintProgress(); });
listen('clean-progress', ({ payload }) => { progress = { kind: 'clean', ...payload }; paintProgress(); });

function renderOverview() {
  if (!status || !junk || busy === 'check') {
    pageEl.innerHTML = progressCard() || `<p class="loading">${esc(t('checking'))}</p>`;
    return;
  }
  const list = findings();
  const score = Math.max(0, 100 - list.reduce((s, f) => s + f.penalty, 0));
  const color = score >= 80 ? 'var(--good)' : score >= 60 ? 'var(--warn)' : 'var(--bad)';
  const label = t(score >= 90 ? 'excellent' : score >= 75 ? 'good' : score >= 50 ? 'fair' : 'poor');
  const circumference = 2 * Math.PI * 78;

  const issues = list.map((f) => {
    const action = f.action ? `<button class="btn small" data-action="${f.action[1]}">${esc(t(f.action[0]))}</button>` : '<span></span>';
    return `<div class="issue ${f.level}"><span class="dot">${svg(ICONS[f.level])}</span>` +
      `<div><b>${esc(f.title)}</b><span>${esc(f.text)}</span></div>${action}</div>`;
  }).join('');

  const on = (startup || []).filter((s) => s.enabled).length;
  const tiles = [
    [t('tileJunk'), bytes(defaultJunk())],
    [t('tileStartup'), `${on}`],
    [t('tileFree', { drive: status.drive }), bytes(status.free)],
    [t('tileUptime'), since(status.uptime)],
    [t('tileMemory'), `${status.memoryLoad}%`],
  ].map(([l, v]) => `<div class="tile"><div class="lbl">${esc(l)}</div><div class="val">${esc(v)}</div></div>`).join('');

  pageEl.innerHTML =
    `<div class="overview"><div class="card score-card">` +
    `<div class="ring"><svg viewBox="0 0 180 180"><circle class="track" cx="90" cy="90" r="78"/>` +
    `<circle class="value" cx="90" cy="90" r="78" stroke="${color}" style="color:${color}" stroke-dasharray="${circumference}" ` +
    `stroke-dashoffset="${circumference * (1 - score / 100)}"/></svg><div class="num">${score}</div></div>` +
    `<div class="score-label" style="color:${color}">${esc(label)}</div>` +
    `<div class="score-sub">${esc(t('lastScan', { time: checkedAt.toLocaleTimeString(locale(), { hour: '2-digit', minute: '2-digit' }) }))}</div>` +
    `<button class="btn primary big" data-action="check">${esc(t('scanNow'))}</button></div>` +
    `<div class="card">${issues}</div></div><div class="care-tiles">${tiles}</div>`;
}

// ---------- cleaner ----------

function renderCleaner() {
  if (!junk) { pageEl.innerHTML = progressCard() || `<p class="loading">${esc(t('scanning'))}</p>`; return; }
  const total = cleanable();
  const locked = junk.some((c) => c.locked);
  const groups = ['windows', 'browsers', 'optional'].map((g) => {
    const rows = junk
      .filter((c) => c.group === g && (g !== 'browsers' || c.files > 0))
      .map((c) => {
        const [name, desc] = pair('cats', c.id);
        const checked = selected.has(c.id) && !c.locked ? ' checked' : '';
        const disabled = c.locked || busy ? ' disabled' : '';
        const tag = c.admin ? `<span class="tag">${esc(t('needsAdmin'))}</span>` : '';
        const count = c.id === 'recycle' ? '' : esc(t('files', { n: num(c.files) }));
        return `<label class="cat${c.locked ? ' locked' : ''}"><input type="checkbox" data-cat="${c.id}"${checked}${disabled}>` +
          `${itemIcon('cleaner', c.id, name)}<div><b>${esc(name)}</b>${tag}<div class="desc">${esc(desc)}</div></div>` +
          `<span class="count">${count}</span><span class="size">${esc(bytes(c.bytes))}</span></label>`;
      }).join('');
    return rows ? `<div class="card"><h3>${esc(t(`group_${g}`))}</h3>${rows}</div>` : '';
  }).join('');

  const result = lastClean
    ? `<div class="card result"><b>${esc(t('freed', { size: bytes(lastClean.freed) }))}</b>` +
      `<div class="note">${esc(t('freedText', { deleted: num(lastClean.deleted), skipped: num(lastClean.skipped) }))}</div></div>`
    : '';
  const banner = locked ? adminBanner(t('adminBanner')) : '';
  const cleanLabel = busy === 'clean' ? t('cleaning') : t('cleanSelected', { size: bytes(total) });

  pageEl.innerHTML =
    `<div class="toolbar"><button class="btn" data-action="scan"${busy ? ' disabled' : ''}>${esc(busy === 'scan' ? t('scanning') : t('scan'))}</button>` +
    `<span class="grow note">${esc(t('closeBrowsers'))}</span>` +
    `<button class="btn primary" data-action="clean"${busy || total === 0 ? ' disabled' : ''}>${esc(cleanLabel)}</button></div>` +
    progressCard() + banner + result + groups;
}

async function runClean() {
  const ids = junk.filter((c) => selected.has(c.id) && !c.locked && c.bytes > 0).map((c) => c.id);
  const total = cleanable();
  if (!ids.length) return;
  const text = t('confirmCleanText') + (ids.includes('recycle') ? `\n${t('recycleWarn')}` : '');
  if (!(await confirmBox(t('confirmClean', { size: bytes(total) }), text, t('clean'), true))) return;
  busy = 'clean';
  progress = null;
  render();
  lastClean = await attempt(() => invoke('clean_junk', { ids }));
  progress = null;
  busy = 'scan';
  render();
  await scanJunk();
  await refreshStatus();
  busy = null;
  render();
}

// ---------- startup ----------

function renderStartup() {
  if (!startup) { pageEl.innerHTML = `<p class="loading">…</p>`; return; }
  const on = startup.filter((s) => s.enabled).length;
  const rows = startup.map((s) => {
    const blocked = s.admin && !admin;
    return `<div class="item with-icon">${itemIcon('startup', s.id, s.name)}<div><div class="name">${esc(s.name)}</div><div class="cmd" title="${esc(s.command)}">${esc(s.command)}</div></div>` +
      `<span class="tag">${esc(t(`loc_${s.location}`))}</span>` +
      `<button class="switch${s.enabled ? ' on' : ''}" data-toggle="${esc(s.id)}"${blocked ? ` disabled title="${esc(t('needsAdmin'))}"` : ''}></button></div>`;
  }).join('');
  const banner = startup.some((s) => s.admin) ? adminBanner(t('adminStartup')) : '';
  pageEl.innerHTML =
    `<div class="toolbar"><b>${esc(t('startupSummary', { on, total: startup.length }))}</b></div>` +
    banner + `<div class="card">${rows}</div><p class="note">${esc(t('startupNote'))}</p>`;
}

async function toggleStartup(id) {
  const item = startup.find((s) => s.id === id);
  if (!item) return;
  const ok = await attempt(() => invoke('set_startup', { id, enabled: !item.enabled }).then(() => true));
  if (ok) startup = await invoke('list_startup');
  render();
}

// ---------- apps ----------

function renderApps() {
  pageEl.innerHTML =
    `<div class="toolbar"><input id="app-search" class="search" type="search" placeholder="${esc(t('search'))}" value="${esc(appsQuery)}">` +
    `<select id="app-sort" class="sel">` +
    ['size', 'name', 'date'].map((s) => `<option value="${s}"${s === appsSort ? ' selected' : ''}>${esc(t(`sort${s[0].toUpperCase()}${s.slice(1)}`))}</option>`).join('') +
    `</select><button class="btn" data-action="refresh-apps">${esc(t('refresh'))}</button></div><div id="app-list"></div>`;
  $('app-search').addEventListener('input', (e) => { appsQuery = e.target.value; renderAppList(); });
  $('app-sort').addEventListener('change', (e) => { appsSort = e.target.value; renderAppList(); });
  renderAppList();
  if (!apps) invoke('list_apps').then((a) => { apps = a; renderAppList(); loadIcons('apps', renderAppList); });
}

function renderAppList() {
  const box = $('app-list');
  if (!box) return;
  if (!apps) { box.innerHTML = '<p class="loading">…</p>'; return; }
  const q = appsQuery.trim().toLowerCase();
  const list = apps
    .filter((a) => !q || a.name.toLowerCase().includes(q) || a.publisher.toLowerCase().includes(q))
    .sort((a, b) => appsSort === 'size' ? b.size - a.size
      : appsSort === 'date' ? b.date.localeCompare(a.date)
        : a.name.localeCompare(b.name, locale()));
  const total = list.reduce((s, a) => s + a.size, 0);
  const rows = list.map((a) =>
    `<div class="item with-icon">${itemIcon('apps', a.id, a.name)}<div><div class="name">${esc(a.name)}</div>` +
    `<div class="sub">${esc([a.publisher, a.version].filter(Boolean).join(' · '))}</div></div>` +
    `<span class="meta">${esc(a.size ? bytes(a.size) : '')}<br>${esc(a.date)}</span>` +
    `<button class="btn small" data-uninstall="${esc(a.id)}">${esc(t('uninstall'))}</button></div>`).join('');
  box.innerHTML = `<p class="note" style="margin:0 0 10px">${esc(t('appsSummary', { n: list.length, size: bytes(total) }))}</p>` +
    `<div class="card">${rows}</div>`;
}

async function uninstall(id) {
  const app = apps.find((a) => a.id === id);
  if (!app) return;
  if (!(await confirmBox(t('confirmUninstall', { name: app.name }), t('confirmUninstallText'), t('uninstall'), true))) return;
  const ok = await attempt(() => invoke('uninstall_app', { id }).then(() => true));
  if (ok) toast(t('uninstallStarted', { name: app.name }));
}

// ---------- optimize ----------

function renderOptimize() {
  const planRows = (plans || []).map((p) =>
    `<label class="plan"><input type="radio" name="plan" data-plan="${esc(p.guid)}"${p.active ? ' checked' : ''}>` +
    `<div><b>${esc(planName(p))}</b><span>${esc(planDescription(p))}</span></div></label>`).join('');
  const hasUltimate = (plans || []).some((p) => ULTIMATE_NAME.test(p.name));
  const tools = TOOLS.map((id) => {
    const [name, desc] = pair('tools', id);
    const label = ADMIN_TOOLS.includes(id) ? t('runAdmin') : t('open');
    return `<div class="tool"><div class="head"><span class="ico">${svg(ICONS[id === 'storage' ? 'storageSense' : id])}</span>${esc(name)}</div>` +
      `<p>${esc(desc)}</p><button class="btn small" data-tool="${id}">${esc(label)}</button></div>`;
  }).join('');
  pageEl.innerHTML =
    `<div class="card"><div class="row-between"><h3>${esc(t('powerPlans'))}</h3>` +
    (hasUltimate ? '' : `<button class="btn small" data-action="ultimate">${esc(t('addUltimate'))}</button>`) +
    `</div>${planRows}</div>` +
    `<div class="card" style="background:transparent;border:0;padding:0"><h3>${esc(t('tools'))}</h3><div class="tools">${tools}</div></div>` +
    `<div class="card"><h3>${esc(t('honestTitle'))}</h3><p class="note" style="margin:0">${esc(t('honest'))}</p></div>`;
}

async function setPlan(guid) {
  const ok = await attempt(() => invoke('set_power_plan', { guid }).then(() => true));
  plans = await invoke('list_power_plans');
  if (ok) toast(t('planSet'));
  render();
}

// ---------- events ----------

pageEl.addEventListener('change', (e) => {
  const box = e.target.closest('[data-cat]');
  if (box) {
    box.checked ? selected.add(box.dataset.cat) : selected.delete(box.dataset.cat);
    render();
  }
  const plan = e.target.closest('[data-plan]');
  if (plan) setPlan(plan.dataset.plan);
});

pageEl.addEventListener('click', async (e) => {
  const el = e.target.closest('[data-action], [data-toggle], [data-uninstall], [data-tool], [data-fd]');
  if (!el || el.disabled) return;
  if (el.dataset.fd) return window.XcareForceDelete.act(el.dataset.fd, Number(el.dataset.index));
  if (el.dataset.toggle) return toggleStartup(el.dataset.toggle);
  if (el.dataset.uninstall) return uninstall(el.dataset.uninstall);
  if (el.dataset.tool) return attempt(() => invoke('run_tool', { tool: el.dataset.tool }));
  const action = el.dataset.action;
  if (action.startsWith('go:')) go(action.slice(3));
  else if (action === 'check') checkAll();
  else if (action === 'clean') runClean();
  else if (action === 'admin') attempt(() => invoke('restart_as_admin'));
  else if (action === 'refresh-apps') {
    apps = null;
    renderAppList();
    apps = await invoke('list_apps');
    loadIcons('apps', renderAppList);
    renderAppList();
  } else if (action === 'ultimate') {
    await attempt(() => invoke('add_ultimate_plan'));
    plans = await invoke('list_power_plans');
    render();
  } else if (action === 'scan') {
    busy = 'scan';
    lastClean = null;
    render();
    await scanJunk();
    busy = null;
    render();
  }
});

Object.assign(PAGES, {
  overview: { render: renderOverview },
  cleaner: { render: renderCleaner },
  startup: { render: renderStartup },
  apps: { render: renderApps },
  optimize: { render: renderOptimize },
  forceDelete: { render: () => window.XcareForceDelete.render(pageEl) },
});

/** First look at the PC when the window opens. */
function startCare() {
  loadIcons('cleaner', repaintPage('cleaner'));
  return checkAll();
}
