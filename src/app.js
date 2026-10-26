'use strict';

// Xpecs window: navigation, information pages, live sensors, benchmarks and
// the HTML report. Hardware data comes from the Rust side ("info" once,
// "sensors" every second).

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const svg = (body) => `<svg class="i" viewBox="0 0 24 24">${body}</svg>`;
const ICONS = {
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

const NAV = ['summary', 'cpu', 'board', 'memory', 'gpu', 'storage', 'network', 'os', '-', 'sensors', 'bench'];

const KIND_COLORS = {
  temp: '#ff8a4c', usage: '#5173d6', thread: '#5173d6', clock: '#8fa9ee', threadclock: '#8fa9ee',
  power: '#f5c451', bytes: '#3dd68c', rate: '#3dd68c', fan: '#4fd1e8', voltage: '#c792ea',
};

const $ = (id) => document.getElementById(id);
const pageEl = $('page');

let info = null;
let current = 'summary';
const readings = new Map(); // id -> { r, min, max, hist }
let readingIds = '';        // ids of the last batch; a change rebuilds the page
const results = {};         // benchmark id -> value
let runningTest = null;
let stressSince = 0;        // Date.now() while the stress test runs

// ---------- formatting ----------

const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

const fixed = (v, d = 0) => v.toLocaleString(locale(), { minimumFractionDigits: d, maximumFractionDigits: d });
const upTo = (v, d) => v.toLocaleString(locale(), { maximumFractionDigits: d });

/** Binary units like Explorer; whole numbers stay whole ("16 GB"). */
function bytes(b) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let i = 0;
  while (b >= 1024 && i < units.length - 1) { b /= 1024; i++; }
  return `${upTo(b, i >= 3 ? 2 : 0)} ${units[i]}`;
}

function duration(s) {
  const d = Math.floor(s / 86400), h = Math.floor((s % 86400) / 3600), m = Math.floor((s % 3600) / 60);
  return d ? `${d}${t('days')} ${h}${t('hours')} ${m}${t('minutes')}` : `${h}${t('hours')} ${m}${t('minutes')}`;
}

/** Steady widths for live values: fixed decimals per unit. */
function fmt(v, unit, style) {
  if (style === 'ghz') return v < 1 ? '—' : `${fixed(v / 1000, 2)} GHz`; // 0 = parked thread
  if (style === 'pct') return `${fixed(v)}%`;
  switch (unit) {
    case '%': return `${fixed(v)} %`;
    case 'MHz': return `${fixed(v)} MHz`;
    case '°C': return `${fixed(v, Number.isInteger(v) ? 0 : 1)} °C`;
    case 'W': return `${fixed(v, 1)} W`;
    case 'V': return `${fixed(v, 3)} V`;
    case 'RPM': return `${fixed(v)} RPM`;
    case 'B': return v >= 1024 ** 3 ? `${fixed(v / 1024 ** 3, 2)} GB` : `${fixed(v / 1024 ** 2)} MB`;
    case 'B/s':
      if (v >= 1024 ** 3) return `${fixed(v / 1024 ** 3, 2)} GB/s`;
      if (v >= 1024 ** 2) return `${fixed(v / 1024 ** 2, 1)} MB/s`;
      return `${fixed(v / 1024)} KB/s`;
    case 's': return duration(v);
    default: return `${fixed(v, 1)} ${unit}`.trim();
  }
}

function toast(text) {
  const el = $('toast');
  el.textContent = text;
  el.classList.add('show');
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => el.classList.remove('show'), 2600);
}

// ---------- sparklines ----------

function spark(canvas, x) {
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth, h = canvas.clientHeight;
  if (!w || !h) return;
  if (canvas.width !== Math.round(w * dpr)) {
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
  }
  const g = canvas.getContext('2d');
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);
  const hist = x.hist.slice(-60);
  if (hist.length < 2) return;
  let lo, hi;
  if (x.r.unit === '%') {
    lo = 0; hi = 100;
  } else {
    lo = Math.min(...hist); hi = Math.max(...hist);
    const pad = Math.max((hi - lo) * 0.15, Math.abs(hi) * 0.02, 0.5);
    lo -= pad; hi += pad;
  }
  const color = KIND_COLORS[x.r.kind] || '#8692ab';
  const step = w / 59;
  const y = (v) => h - 2 - ((v - lo) / (hi - lo)) * (h - 4);
  const x0 = w - (hist.length - 1) * step;
  g.beginPath();
  hist.forEach((v, i) => (i ? g.lineTo(x0 + i * step, y(v)) : g.moveTo(x0, y(v))));
  g.strokeStyle = color;
  g.lineWidth = 1.5;
  g.lineJoin = 'round';
  g.stroke();
  g.lineTo(w, h);
  g.lineTo(x0, h);
  g.closePath();
  g.globalAlpha = 0.14;
  g.fillStyle = color;
  g.fill();
  g.globalAlpha = 1;
}

// ---------- live values on the current page ----------

function updateLive() {
  for (const el of pageEl.querySelectorAll('[data-v]')) {
    const x = readings.get(el.dataset.v);
    if (!x) continue;
    el.textContent = fmt(x.r.value, x.r.unit, el.dataset.f);
    if (x.r.unit === '°C') el.classList.toggle('hot', x.r.value >= 85);
  }
  for (const el of pageEl.querySelectorAll('[data-min]')) {
    const x = readings.get(el.dataset.min);
    if (x) el.textContent = fmt(x.min, x.r.unit);
  }
  for (const el of pageEl.querySelectorAll('[data-max]')) {
    const x = readings.get(el.dataset.max);
    if (x) el.textContent = fmt(x.max, x.r.unit);
  }
  for (const el of pageEl.querySelectorAll('[data-bar]')) {
    const x = readings.get(el.dataset.bar);
    if (x) el.style.height = `${Math.min(100, x.r.value)}%`;
  }
  for (const el of pageEl.querySelectorAll('canvas[data-spark]')) {
    const x = readings.get(el.dataset.spark);
    if (x) spark(el, x);
  }
  const timer = pageEl.querySelector('[data-stress]');
  if (timer) timer.textContent = stressSince ? duration24((Date.now() - stressSince) / 1000) : '0:00:00';
}

function duration24(s) {
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = Math.floor(s % 60);
  return `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`;
}

function tempClass(x) {
  return x.r.unit === '°C' ? ' k-temp' : '';
}

function tile(id, label) {
  const x = readings.get(id);
  if (!x) return '';
  const spk = x.r.kind === 'time' ? '' : `<canvas data-spark="${esc(id)}"></canvas>`;
  return `<div class="tile"><div class="lbl" title="${esc(x.r.group)}">${esc(label ?? tr(x.r.name))}</div>` +
    `<div class="val${tempClass(x)}" data-v="${esc(id)}"></div>${spk}</div>`;
}

// ---------- pages ----------

let pendingUpdate = null; // newer release on GitHub, from check_update

function renderUpdateBar() {
  const bar = $('update-bar');
  bar.hidden = !pendingUpdate;
  if (!pendingUpdate) return;
  bar.innerHTML = `<span>${esc(t('updateAvailable', { v: pendingUpdate.version }))}</span>` +
    `<button class="btn small primary" id="update-now">${esc(t('updateNow'))}</button>`;
  $('update-now').onclick = async (e) => {
    e.target.disabled = true;
    e.target.textContent = t('updating');
    try {
      await invoke('install_update'); // the app restarts on success
    } catch (err) {
      toast(t('updateFailed', { error: err }));
      renderUpdateBar();
    }
  };
}

function renderNav() {
  renderUpdateBar();
  $('nav').innerHTML = NAV.map((id) => id === '-'
    ? '<div class="nav-sep"></div>'
    : `<button class="nav-item${id === current ? ' on' : ''}" data-page="${id}">${svg(ICONS[id])}<span>${esc(t(id))}</span></button>`).join('');
  for (const el of document.querySelectorAll('[data-t]')) el.textContent = t(el.dataset.t);
  $('export').innerHTML = `${svg(ICONS.export)}<span>${esc(t('export'))}</span>`;
}

function go(page) {
  current = page;
  renderNav();
  pageEl.scrollTop = 0;
  render();
}

function render() {
  $('title').textContent = t(current);
  if (current === 'summary') renderSummary();
  else if (current === 'sensors') renderSensors();
  else if (current === 'bench') renderBench();
  else renderSection(current);
  updateLive();
}

const loading = (key) => `<p class="loading">${esc(t(key))}</p>`;

/** Shorter, friendlier hardware names for the summary cards. */
function cleanName(name) {
  return String(name || '')
    .replace(/\((R|TM|tm|r)\)/g, '')
    .replace(/\b\d+(st|nd|rd|th) Gen\s+/i, '')
    .replace(/Micro-Star International Co\.?,? Ltd\.?/i, 'MSI')
    .replace(/ASUSTeK COMPUTER INC\.?/i, 'ASUS')
    .replace(/Gigabyte Technology Co\.?,? Ltd\.?/i, 'Gigabyte')
    .replace(/ASRock Incorporation/i, 'ASRock')
    .replace(/\s+/g, ' ')
    .trim();
}

function summaryCard(page, title, main, subs) {
  return `<button class="sum-card" data-go="${page}"><span class="ico">${svg(ICONS[page])}</span><span>` +
    `<div class="t">${esc(title)}</div><div class="m">${esc(main || t('noData'))}</div>` +
    subs.filter(Boolean).map((s) => `<div class="s">${esc(s)}</div>`).join('') + '</span></button>';
}

function renderSummary() {
  if (!info) { pageEl.innerHTML = loading('loading'); return; }
  const s = info.summary;
  const cpu = s.cpu || {};
  const mem = s.memory || {};
  const gpu = s.gpu || {};
  const board = s.board || {};
  const os = s.os || {};
  const disks = s.storage || [];
  const net = s.network;

  const cores = cpu.efficient
    ? t('coresHybrid', { cores: cpu.cores, p: cpu.performance, e: cpu.efficient, threads: cpu.threads })
    : t('cores', { cores: cpu.cores, threads: cpu.threads });
  const adapters = gpu.adapters || [];
  const cards = [
    summaryCard('cpu', t('cpu'), cleanName(cpu.name), [cores, cpu.baseMhz ? t('base', { ghz: `${fixed(cpu.baseMhz / 1000, 2)} GHz` }) : '']),
    summaryCard('gpu', t('gpu'), adapters.map((a) => a.name).join(' + '), [
      adapters[0]?.vram ? t('vram', { size: bytes(adapters[0].vram) }) : '',
      gpu.displays?.length ? (gpu.displays.length === 1 ? t('display1') : t('displayCount', { n: gpu.displays.length })) : '',
    ]),
    summaryCard('memory', t('memory'), mem.total ? `${bytes(mem.total)} ${mem.type || ''}` : '', [
      mem.modules ? t('modules', { n: mem.modules, size: bytes(mem.moduleSize), speed: mem.speed }) : '',
    ]),
    summaryCard('board', t('board'), cleanName(board.board), [
      board.bios ? `BIOS ${board.bios}${board.biosDate ? ` · ${board.biosDate}` : ''}` : '',
    ]),
    summaryCard('storage', t('storage'), disks[0] ? `${disks[0].name}` : '',
      disks.map((d, i) => (i === 0 ? `${bytes(d.size)} · ${[d.bus, d.media].filter(Boolean).join(' ')}` : `${d.name} · ${bytes(d.size)}`)).slice(0, 5)),
    summaryCard('os', t('os'), os.name, [[os.version, os.build && t('build', { b: os.build })].filter(Boolean).join(' · '), os.arch]),
    summaryCard('network', t('network'), cleanName(net?.name), [net?.ipv4, net?.speed]),
  ];

  const firstDiskTemp = [...readings.keys()].find((id) => /^disk\d+\.temp$/.test(id));
  const tiles = [
    tile('cpu.usage', t('cpuUsage')),
    tile('cpu.clock', t('cpuClock')),
    tile('mem.usage', t('memUsage')),
    tile('gpu0.temp', t('gpuTemp')),
    tile('gpu0.usage', t('gpuUsage')),
    tile('gpu0.power', t('gpuPower')),
    firstDiskTemp ? tile(firstDiskTemp, t('diskTemp')) : '',
  ].join('');

  pageEl.innerHTML =
    `<div class="hero"><b>${esc(board.computer || '')}</b><span>${esc(os.name || '')}</span>` +
    `<span>${esc(tr('Uptime'))}: <span data-v="sys.uptime"></span></span></div>` +
    (tiles ? `<div class="tiles">${tiles}</div>` : '') +
    `<div class="cards">${cards.join('')}</div>`;
}

// Which live readings each information page shows as tiles on top.
const LIVE = {
  cpu: (id) => ['cpu.usage', 'cpu.clock', 'cpu.clock.max'].includes(id) || id.startsWith('acpi.'),
  memory: (id) => id.startsWith('mem.'),
  gpu: (id) => /^gpu\d+\./.test(id),
};

// Performance counters write "Intel[R] … #2" where WMI writes "Intel(R) … #2".
const netKey = (s) => s.toLowerCase().replace(/\[/g, '(').replace(/\]/g, ')').replace(/_/g, '#');

/** Live rows that belong inside one card: a disk's speeds, an adapter's traffic. */
function liveRows(sectionId, g) {
  let ids = [];
  if (sectionId === 'storage') {
    const disk = g.rows.find(([k]) => k === 'Disk number')?.[1];
    if (disk != null) ids = ['temp', 'read', 'write', 'active'].map((k) => `disk${disk}.${k}`);
  } else if (sectionId === 'network') {
    const key = netKey(g.title);
    ids = [...readings.values()].filter((x) => x.r.id.startsWith('net.') && netKey(x.r.group) === key).map((x) => x.r.id);
  }
  return ids.filter((id) => readings.has(id)).map((id) => {
    const x = readings.get(id);
    return `<tr><td>${esc(tr(x.r.name))}</td><td class="lv${tempClass(x)}" data-v="${esc(id)}"></td></tr>`;
  }).join('');
}

function groupCard(g, live = '') {
  const rows = g.rows.map(([k, v]) => `<tr><td>${esc(tr(k))}</td><td class="v">${esc(tr(v))}</td></tr>`).join('');
  return `<div class="card"><h3>${esc(tr(g.title))}</h3><table class="kv">${live}${rows}</table></div>`;
}

function renderSection(id) {
  if (!info) { pageEl.innerHTML = loading('loading'); return; }
  const section = info.sections.find((s) => s.id === id);
  const live = LIVE[id];
  const tiles = live ? [...readings.values()].filter((x) => live(x.r.id)).map((x) => tile(x.r.id)).join('') : '';
  pageEl.innerHTML =
    (tiles ? `<div class="tiles">${tiles}</div>` : '') +
    (id === 'cpu' ? threadCards() : '') +
    `<div class="columns">${(section?.groups || []).map((g) => groupCard(g, liveRows(id, g))).join('')}</div>`;
}

/** Columns that split `n` cells into even rows of at most `max`. */
const evenColumns = (n, max) => Math.ceil(n / Math.ceil(n / max));

/** Per-thread usage bars and clocks. */
function threadCards() {
  const list = [...readings.values()];
  const usage = list.filter((x) => x.r.kind === 'thread');
  const clocks = list.filter((x) => x.r.kind === 'threadclock');
  let html = '';
  if (usage.length) {
    html += `<div class="card"><h3>${esc(tr('CPU threads'))}</h3>` +
      `<div class="bars" style="grid-template-columns:repeat(${evenColumns(usage.length, 32)},1fr)">` +
      usage.map((x) => `<div class="bar"><div class="track"><div class="fill" data-bar="${x.r.id}"></div></div>` +
        `<b data-v="${x.r.id}" data-f="pct"></b>${esc(x.r.name)}</div>`).join('') + '</div></div>';
  }
  if (clocks.length) {
    html += `<div class="card"><h3>${esc(tr('CPU thread clocks'))}</h3>` +
      `<div class="clocks" style="grid-template-columns:repeat(${evenColumns(clocks.length, 8)},1fr)">` +
      clocks.map((x) => `<div class="clock"><b data-v="${x.r.id}" data-f="ghz"></b>${esc(x.r.name)}</div>`).join('') +
      '</div></div>';
  }
  return html;
}

function groupRank(x) {
  const id = x.r.id;
  if (id.startsWith('cpu.') && x.r.kind !== 'thread' && x.r.kind !== 'threadclock') return 0;
  if (id.startsWith('mem.')) return 1;
  if (id.startsWith('gpu')) return 2;
  if (id.startsWith('lhm')) return 3;
  if (id.startsWith('acpi.')) return 4;
  if (id.startsWith('disk')) return 5;
  if (id.startsWith('net.')) return 6;
  if (id.startsWith('sys.')) return 7;
  return 8;
}

function renderSensors() {
  if (!readings.size) { pageEl.innerHTML = loading('waiting'); return; }
  const groups = new Map();
  for (const x of [...readings.values()].sort((a, b) => groupRank(a) - groupRank(b))) {
    if (x.r.kind === 'thread' || x.r.kind === 'threadclock') continue;
    if (!groups.has(x.r.group)) groups.set(x.r.group, []);
    groups.get(x.r.group).push(x);
  }
  const hasLhm = [...readings.keys()].some((id) => id.startsWith('lhm'));
  const head = `<tr><th>${esc(t('sensor'))}</th><th>${esc(t('current'))}</th><th>${esc(t('min'))}</th><th>${esc(t('max'))}</th><th></th></tr>`;
  const tables = [...groups].map(([group, list]) => {
    const rows = list.map((x) => {
      const id = esc(x.r.id);
      const sparkCell = x.r.kind === 'time' ? '' : `<canvas data-spark="${id}"></canvas>`;
      return `<tr><td title="${esc(tr(x.r.name))}">${esc(tr(x.r.name))}</td><td class="cur${tempClass(x)}" data-v="${id}"></td>` +
        `<td class="mm" data-min="${id}"></td><td class="mm" data-max="${id}"></td><td class="sp">${sparkCell}</td></tr>`;
    }).join('');
    return `<div class="card"><h3>${esc(tr(group))}</h3><table class="sensors">${head}${rows}</table></div>`;
  }).join('');

  pageEl.innerHTML =
    `<div class="card-head" style="margin-bottom:14px"><span></span><button class="btn small" data-act="reset">${esc(t('reset'))}</button></div>` +
    (hasLhm ? '' : `<div class="card note">${esc(t('lhmNote'))}</div>`) +
    threadCards() +
    `<div class="columns">${tables}</div>`;
}

const TESTS = [
  {
    title: 'memory',
    note: 'memNote',
    items: [
      { id: 'mem-read', label: 'memRead', unit: 'GB/s', digits: 1 },
      { id: 'mem-write', label: 'memWrite', unit: 'GB/s', digits: 1 },
      { id: 'mem-copy', label: 'memCopy', unit: 'GB/s', digits: 1 },
      { id: 'mem-latency', label: 'memLatency', unit: 'ns', digits: 1, lower: true, note: 'latNote' },
    ],
  },
  {
    title: 'cpu',
    items: [
      { id: 'cpu-int-1', label: 'cpuInt1', unit: 'Mop/s', digits: 0, note: 'intNote' },
      { id: 'cpu-int-n', label: 'cpuIntN', unit: 'Mop/s', digits: 0, single: 'cpu-int-1' },
      { id: 'cpu-fp-1', label: 'cpuFp1', unit: 'Miter/s', digits: 0, note: 'fpNote' },
      { id: 'cpu-fp-n', label: 'cpuFpN', unit: 'Miter/s', digits: 0, single: 'cpu-fp-1' },
    ],
  },
];

function renderBench() {
  const busy = !!runningTest || !!stressSince;
  const cards = TESTS.map((group) => {
    const rows = group.items.map((test) => {
      const value = results[test.id];
      let res;
      if (runningTest === test.id) res = `<span class="res wait">${esc(t('running'))}</span>`;
      else if (value == null) res = `<span class="res wait">${esc(t('notRun'))}</span>`;
      else res = `<span class="res">${fixed(value, test.digits)} ${test.unit}</span>`;
      const scaling = test.single && value && results[test.single]
        ? ` · ${t('scaling', { x: fixed(value / results[test.single], 1) })}` : '';
      const sub = [test.note && t(test.note), t(test.lower ? 'lower' : 'higher')].filter(Boolean).join(' · ') + scaling;
      return `<div class="bench-row"><div class="name">${esc(t(test.label))}<small>${esc(sub)}</small></div>${res}` +
        `<button class="btn small" data-run="${test.id}"${busy ? ' disabled' : ''}>${esc(t('run'))}</button></div>`;
    }).join('');
    const note = group.note ? `<p class="note" style="margin:0 0 6px">${esc(t(group.note))}</p>` : '';
    return `<div class="card"><h3>${esc(t(group.title))}</h3>${note}${rows}</div>`;
  }).join('');

  const temps = [...readings.values()]
    .filter((x) => x.r.kind === 'temp' && !x.r.id.startsWith('disk'))
    .map((x) => tile(x.r.id, `${tr(x.r.name)} · ${x.r.group}`)).join('');
  const stress =
    `<div class="card"><div class="card-head"><h3>${esc(t('stress'))}</h3>` +
    `<button class="btn ${stressSince ? 'danger' : 'primary'}" data-act="stress"${runningTest ? ' disabled' : ''}>` +
    `${esc(t(stressSince ? 'stop' : 'start'))}</button></div>` +
    `<p class="note" style="margin-top:0">${esc(t('stressNote'))}</p>` +
    `<div class="stress-time" data-stress></div><div class="tiles" style="margin:12px 0 0">` +
    [tile('cpu.usage', t('cpuUsage')), tile('cpu.clock', t('cpuClock')), tile('cpu.clock.max'), temps].join('') +
    '</div></div>';

  pageEl.innerHTML =
    `<div class="card-head" style="margin-bottom:14px"><span class="note">${stressSince ? esc(t('busyStress')) : ''}</span>` +
    `<button class="btn primary" data-act="run-all"${busy ? ' disabled' : ''}>${esc(t('runAll'))}</button></div>` +
    `<div class="columns">${cards}${stress}</div>`;
}

async function runTests(ids) {
  for (const id of ids) {
    runningTest = id;
    if (current === 'bench') render();
    try {
      results[id] = await invoke('run_benchmark', { test: id });
    } catch (e) {
      toast(String(e));
    }
  }
  runningTest = null;
  if (current === 'bench') render();
}

async function toggleStress() {
  const run = !stressSince;
  await invoke('set_stress', { running: run });
  stressSince = run ? Date.now() : 0;
  render();
}

// ---------- report ----------

function reportHtml() {
  const now = new Date();
  const table = (rows) => `<table>${rows.map((r) => `<tr>${r.map((c) => `<td>${esc(c)}</td>`).join('')}</tr>`).join('')}</table>`;
  let body = `<h1>Xpecs · ${esc(info.summary.board?.computer || '')}</h1>` +
    `<p class="meta">${esc(t('generated'))} ${esc(now.toLocaleString(locale()))}</p>`;
  for (const section of info.sections) {
    body += `<h2>${esc(t(section.id))}</h2>`;
    for (const g of section.groups) {
      body += `<h3>${esc(tr(g.title))}</h3>` + table(g.rows.map(([k, v]) => [tr(k), tr(v)]));
    }
  }
  if (readings.size) {
    body += `<h2>${esc(t('snapshot'))}</h2>` + table([
      ['', t('sensor'), t('current'), t('min'), t('max')],
      ...[...readings.values()].sort((a, b) => groupRank(a) - groupRank(b)).map((x) => [
        tr(x.r.group), tr(x.r.name), fmt(x.r.value, x.r.unit), fmt(x.min, x.r.unit), fmt(x.max, x.r.unit),
      ]),
    ]);
  }
  const done = TESTS.flatMap((g) => g.items).filter((test) => results[test.id] != null);
  if (done.length) {
    body += `<h2>${esc(t('results'))}</h2>` +
      table(done.map((test) => [t(test.label), `${fixed(results[test.id], test.digits)} ${test.unit}`]));
  }
  const css = 'body{font:14px/1.5 "Segoe UI",system-ui,sans-serif;color:#18213a;max-width:980px;margin:32px auto;padding:0 24px}' +
    'h1{color:#1c53a3;margin-bottom:0}h2{color:#1c53a3;border-bottom:2px solid #dfe6f5;padding-bottom:4px;margin-top:32px}' +
    'h3{margin:18px 0 6px;font-size:14px;color:#3b5bbe}.meta{color:#6b7488}table{border-collapse:collapse;width:100%}' +
    'td{padding:5px 10px 5px 0;border-bottom:1px solid #eef1f7;vertical-align:top}td:first-child{color:#6b7488;width:34%}';
  return `<!doctype html><html lang="${language}"><head><meta charset="utf-8"><title>Xpecs report</title>` +
    `<style>${css}</style></head><body>${body}<footer style="margin-top:40px;padding-top:12px;border-top:1px solid #dfe6f5;color:#6b7488;font-size:12px">Xpecs · <a href="https://skale.club">Skale Club</a></footer></body></html>`;
}

async function exportReport() {
  if (!info) return;
  const d = new Date();
  const p = (n) => String(n).padStart(2, '0');
  const stamp = `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}`;
  const name = `xpecs-${info.summary.board?.computer || 'pc'}-${stamp}`;
  try {
    const path = await invoke('save_report', { name, html: reportHtml() });
    toast(t('saved', { path }));
  } catch (e) {
    toast(t('failed', { error: e }));
  }
}

// ---------- events ----------

$('nav').addEventListener('click', (e) => {
  const item = e.target.closest('[data-page]');
  if (item) go(item.dataset.page);
});

pageEl.addEventListener('click', (e) => {
  const target = e.target.closest('[data-go], [data-run], [data-act], td.v');
  if (!target) return;
  if (target.dataset.go) go(target.dataset.go);
  else if (target.dataset.run) runTests([target.dataset.run]);
  else if (target.dataset.act === 'run-all') runTests(TESTS.flatMap((g) => g.items.map((i) => i.id)));
  else if (target.dataset.act === 'stress') toggleStress();
  else if (target.dataset.act === 'reset') {
    for (const x of readings.values()) x.min = x.max = x.r.value;
    updateLive();
  } else if (target.matches('td.v')) {
    navigator.clipboard.writeText(target.textContent).then(() => toast(t('copied')));
  }
});

$('export').addEventListener('click', exportReport);
$('skale').addEventListener('click', () => invoke('open_site'));

$('lang').addEventListener('change', (e) => {
  setLanguage(e.target.value);
  invoke('set_language', { language });
  renderNav();
  render();
});

window.addEventListener('resize', updateLive);
setInterval(() => { if (stressSince && current === 'bench') updateLive(); }, 1000);

listen('info', ({ payload }) => {
  info = payload;
  render();
});

listen('sensors', ({ payload }) => {
  for (const r of payload) {
    let x = readings.get(r.id);
    if (!x) {
      x = { r, min: r.value, max: r.value, hist: [] };
      readings.set(r.id, x);
    }
    x.r = r;
    x.min = Math.min(x.min, r.value);
    x.max = Math.max(x.max, r.value);
    x.hist.push(r.value);
    if (x.hist.length > 90) x.hist.shift();
  }
  const ids = payload.map((r) => r.id).join('|');
  if (ids !== readingIds && !runningTest) {
    readingIds = ids;
    render();
  } else {
    updateLive();
  }
});

(async () => {
  try {
    setLanguage((await invoke('get_settings')).language);
  } catch (_) { /* English */ }
  $('lang').value = language;
  renderNav();
  window.__TAURI__.app.getVersion().then((v) => { document.querySelector('.ver').textContent = `v${v}`; });
  invoke('check_update').then((u) => { pendingUpdate = u; renderUpdateBar(); });
  info = await invoke('get_info');
  render();
})();
