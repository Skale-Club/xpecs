'use strict';

// Drivers page: devices with problems, driver updates from Windows Update
// (installed after a restore point, as administrator), the graphics driver
// with a link to its maker's own app, every installed driver, and a backup
// of all third-party drivers. Drivers never come from anywhere but Windows
// Update.

addTexts({
  en: {
    ui: {
      drivers: 'Drivers', drvTitle: 'Drivers',
      drvProblemsFound: '{n} devices need attention.', drvProblemsFoundOne: '1 device needs attention.',
      drvLoading: 'Reading the drivers…',
      drvProblems: 'Devices with problems', drvNoProblems: 'No device reports a problem.',
      drvProblemsHint: 'Windows Update may have the missing driver: search below. Device Manager shows more.',
      drvOpenDevmgmt: 'Device Manager',
      drvUpdates: 'Updates from Windows Update',
      drvUpdatesIntro: 'The same signed drivers Windows offers in Optional updates. Searching takes 10 to 60 seconds.',
      drvSearch: 'Search for updates', drvSearching: 'Searching Windows Update…', drvSearchAgain: 'Search again',
      drvNoUpdates: 'Windows Update has no newer drivers for this PC.',
      drvInstall: 'Install {n} selected', drvInstallOne: 'Install 1 selected', drvInstalling: 'Installing… this can take a few minutes.',
      drvInstallAdmin: 'Installing drivers needs administrator rights.',
      drvConfirm: 'Install {n} driver updates?', drvConfirmOne: 'Install 1 driver update?',
      drvConfirmText: 'A restore point is created first, so you can go back. Devices may flicker or reconnect while their driver changes, and Windows may ask to restart.',
      drvNoRestorePoint: 'No restore point',
      drvNoRestorePointText: 'Windows could not create a restore point ({error}). System Restore may be turned off. Install without one?',
      drvInstallAnyway: 'Install anyway',
      drvResult: 'Result', drvResultInstalled: 'Installed', drvResultFailed: 'Failed', drvResultSkipped: 'Needs Windows Update (license)',
      drvRestoreCreated: 'Restore point created before installing.', drvRestoreRecent: 'Windows already made a restore point in the last day; it covers this install.',
      drvRestoreSkipped: 'Installed without a restore point.',
      drvReboot: 'Restart Windows to finish installing.',
      drvGraphics: 'Graphics driver',
      drvGraphicsNote: 'Graphics drivers reach Windows Update late. Get the newest from the maker’s app.',
      drvVendorApp: 'Open {name} drivers',
      drvInstalled: 'Installed drivers', drvThirdParty: 'From manufacturers', drvAll: 'All',
      drvCount: '{n} drivers', drvCountOne: '1 driver', drvSearchList: 'Search drivers',
      drvBackup: 'Back up drivers',
      drvBackupText: 'Copies every manufacturer driver to Data\\driver-backup, to put back after reinstalling Windows (Device Manager › Update driver › Browse).',
      drvBackupRun: 'Back up now', drvBackingUp: 'Backing up…', drvBackedUp: 'Drivers copied to {path}',
      drvBackupAdmin: 'Backing up drivers needs administrator rights.',
      drvHonest: 'An old date doesn’t mean a driver is outdated: many are final. Xpecs never downloads drivers from other sites.',
      drvVersion: 'Version', drvDate: 'Date', drvProvider: 'Provider',
      drvError_needsAdmin: 'This needs administrator rights.', drvError_noUpdates: 'Choose at least one update.',
      drvError_backupFailed: 'Windows could not copy the drivers.',
      drvSearchFailed: 'Windows Update did not answer: {error}',
    },
    problems: {
      1: 'Not configured correctly', 3: 'Driver may be corrupted', 10: 'Cannot start', 12: 'Not enough free resources',
      14: 'Restart Windows to finish', 18: 'Reinstall the driver', 19: 'Registry settings damaged', 21: 'Being removed',
      22: 'Disabled', 24: 'Not present or not working', 28: 'No driver installed', 31: 'Not working properly',
      39: 'Driver missing or damaged', 43: 'Stopped by Windows after a problem', 45: 'Not connected', 52: 'Driver not signed',
    },
  },
  pt: {
    ui: {
      drivers: 'Drivers', drvTitle: 'Drivers',
      drvProblemsFound: '{n} dispositivos precisam de atenção.', drvProblemsFoundOne: '1 dispositivo precisa de atenção.',
      drvLoading: 'Lendo os drivers…',
      drvProblems: 'Dispositivos com problema', drvNoProblems: 'Nenhum dispositivo relata problema.',
      drvProblemsHint: 'O Windows Update pode ter o driver que falta: busque abaixo. O Gerenciador de Dispositivos mostra mais.',
      drvOpenDevmgmt: 'Gerenciador de Dispositivos',
      drvUpdates: 'Atualizações do Windows Update',
      drvUpdatesIntro: 'Os mesmos drivers assinados que o Windows oferece em Atualizações opcionais. A busca leva de 10 a 60 segundos.',
      drvSearch: 'Buscar atualizações', drvSearching: 'Buscando no Windows Update…', drvSearchAgain: 'Buscar de novo',
      drvNoUpdates: 'O Windows Update não tem drivers mais novos para este PC.',
      drvInstall: 'Instalar {n} selecionados', drvInstallOne: 'Instalar 1 selecionado', drvInstalling: 'Instalando… pode levar alguns minutos.',
      drvInstallAdmin: 'Instalar drivers precisa de administrador.',
      drvConfirm: 'Instalar {n} atualizações de driver?', drvConfirmOne: 'Instalar 1 atualização de driver?',
      drvConfirmText: 'Antes é criado um ponto de restauração, para poder voltar atrás. Os dispositivos podem piscar ou reconectar enquanto o driver muda, e o Windows pode pedir para reiniciar.',
      drvNoRestorePoint: 'Sem ponto de restauração',
      drvNoRestorePointText: 'O Windows não conseguiu criar um ponto de restauração ({error}). A Restauração do Sistema pode estar desligada. Instalar mesmo assim?',
      drvInstallAnyway: 'Instalar mesmo assim',
      drvResult: 'Resultado', drvResultInstalled: 'Instalado', drvResultFailed: 'Falhou', drvResultSkipped: 'Precisa do Windows Update (licença)',
      drvRestoreCreated: 'Ponto de restauração criado antes de instalar.', drvRestoreRecent: 'O Windows já criou um ponto de restauração no último dia; ele cobre esta instalação.',
      drvRestoreSkipped: 'Instalado sem ponto de restauração.',
      drvReboot: 'Reinicie o Windows para terminar a instalação.',
      drvGraphics: 'Driver de vídeo',
      drvGraphicsNote: 'Drivers de vídeo chegam atrasados no Windows Update. Pegue o mais novo no app do fabricante.',
      drvVendorApp: 'Abrir drivers da {name}',
      drvInstalled: 'Drivers instalados', drvThirdParty: 'De fabricantes', drvAll: 'Todos',
      drvCount: '{n} drivers', drvCountOne: '1 driver', drvSearchList: 'Buscar drivers',
      drvBackup: 'Backup dos drivers',
      drvBackupText: 'Copia todos os drivers de fabricantes para Data\\driver-backup, para recolocar depois de reinstalar o Windows (Gerenciador de Dispositivos › Atualizar driver › Procurar).',
      drvBackupRun: 'Fazer backup', drvBackingUp: 'Fazendo backup…', drvBackedUp: 'Drivers copiados para {path}',
      drvBackupAdmin: 'O backup dos drivers precisa de administrador.',
      drvHonest: 'Data antiga não quer dizer driver desatualizado: muitos são versões finais. O Xpecs nunca baixa drivers de outros sites.',
      drvVersion: 'Versão', drvDate: 'Data', drvProvider: 'Fornecedor',
      drvError_needsAdmin: 'Isto precisa de administrador.', drvError_noUpdates: 'Escolha pelo menos uma atualização.',
      drvError_backupFailed: 'O Windows não conseguiu copiar os drivers.',
      drvSearchFailed: 'O Windows Update não respondeu: {error}',
    },
    problems: {
      1: 'Não configurado corretamente', 3: 'O driver pode estar corrompido', 10: 'Não consegue iniciar', 12: 'Recursos insuficientes',
      14: 'Reinicie o Windows para concluir', 18: 'Reinstale o driver', 19: 'Configurações do registro danificadas', 21: 'Sendo removido',
      22: 'Desativado', 24: 'Ausente ou sem funcionar', 28: 'Sem driver instalado', 31: 'Não está funcionando direito',
      39: 'Driver ausente ou danificado', 43: 'Parado pelo Windows após um problema', 45: 'Desconectado', 52: 'Driver sem assinatura',
    },
  },
  es: {
    ui: {
      drivers: 'Controladores', drvTitle: 'Controladores',
      drvProblemsFound: '{n} dispositivos necesitan atención.', drvProblemsFoundOne: '1 dispositivo necesita atención.',
      drvLoading: 'Leyendo los controladores…',
      drvProblems: 'Dispositivos con problemas', drvNoProblems: 'Ningún dispositivo informa problemas.',
      drvProblemsHint: 'Windows Update puede tener el controlador que falta: busca abajo. El Administrador de dispositivos muestra más.',
      drvOpenDevmgmt: 'Administrador de dispositivos',
      drvUpdates: 'Actualizaciones de Windows Update',
      drvUpdatesIntro: 'Los mismos controladores firmados que Windows ofrece en Actualizaciones opcionales. La búsqueda tarda de 10 a 60 segundos.',
      drvSearch: 'Buscar actualizaciones', drvSearching: 'Buscando en Windows Update…', drvSearchAgain: 'Buscar de nuevo',
      drvNoUpdates: 'Windows Update no tiene controladores más nuevos para este PC.',
      drvInstall: 'Instalar {n} seleccionados', drvInstallOne: 'Instalar 1 seleccionado', drvInstalling: 'Instalando… puede tardar unos minutos.',
      drvInstallAdmin: 'Instalar controladores requiere administrador.',
      drvConfirm: '¿Instalar {n} actualizaciones de controladores?', drvConfirmOne: '¿Instalar 1 actualización de controlador?',
      drvConfirmText: 'Antes se crea un punto de restauración para poder volver atrás. Los dispositivos pueden parpadear o reconectarse mientras cambia el controlador, y Windows puede pedir reiniciar.',
      drvNoRestorePoint: 'Sin punto de restauración',
      drvNoRestorePointText: 'Windows no pudo crear un punto de restauración ({error}). Restaurar sistema puede estar desactivado. ¿Instalar igualmente?',
      drvInstallAnyway: 'Instalar igualmente',
      drvResult: 'Resultado', drvResultInstalled: 'Instalado', drvResultFailed: 'Falló', drvResultSkipped: 'Requiere Windows Update (licencia)',
      drvRestoreCreated: 'Punto de restauración creado antes de instalar.', drvRestoreRecent: 'Windows ya creó un punto de restauración en el último día; cubre esta instalación.',
      drvRestoreSkipped: 'Instalado sin punto de restauración.',
      drvReboot: 'Reinicia Windows para terminar la instalación.',
      drvGraphics: 'Controlador de gráficos',
      drvGraphicsNote: 'Los controladores de gráficos llegan tarde a Windows Update. Consigue el más nuevo en la app del fabricante.',
      drvVendorApp: 'Abrir controladores de {name}',
      drvInstalled: 'Controladores instalados', drvThirdParty: 'De fabricantes', drvAll: 'Todos',
      drvCount: '{n} controladores', drvCountOne: '1 controlador', drvSearchList: 'Buscar controladores',
      drvBackup: 'Copia de seguridad',
      drvBackupText: 'Copia todos los controladores de fabricantes a Data\\driver-backup, para volver a ponerlos tras reinstalar Windows (Administrador de dispositivos › Actualizar controlador › Examinar).',
      drvBackupRun: 'Hacer copia', drvBackingUp: 'Copiando…', drvBackedUp: 'Controladores copiados a {path}',
      drvBackupAdmin: 'La copia de los controladores requiere administrador.',
      drvHonest: 'Una fecha antigua no significa que el controlador esté desactualizado: muchos son versiones finales. Xpecs nunca descarga controladores de otros sitios.',
      drvVersion: 'Versión', drvDate: 'Fecha', drvProvider: 'Proveedor',
      drvError_needsAdmin: 'Esto requiere administrador.', drvError_noUpdates: 'Elige al menos una actualización.',
      drvError_backupFailed: 'Windows no pudo copiar los controladores.',
      drvSearchFailed: 'Windows Update no respondió: {error}',
    },
    problems: {
      1: 'No configurado correctamente', 3: 'El controlador puede estar dañado', 10: 'No se puede iniciar', 12: 'Recursos insuficientes',
      14: 'Reinicia Windows para terminar', 18: 'Reinstala el controlador', 19: 'Configuración del registro dañada', 21: 'Se está quitando',
      22: 'Deshabilitado', 24: 'Ausente o sin funcionar', 28: 'Sin controlador instalado', 31: 'No funciona correctamente',
      39: 'Controlador ausente o dañado', 43: 'Detenido por Windows tras un problema', 45: 'Desconectado', 52: 'Controlador sin firma',
    },
  },
});

window.XpecsDrivers = (() => {
  let inventory = null;     // { drivers, problems }
  let loadError = '';
  let updates = null;       // list from Windows Update
  let searching = false;
  let searchError = '';
  let picked = new Set();
  let installing = false;
  let outcome = null;
  let backingUp = false;
  let showAll = false;
  let query = '';

  const paint = () => { if (current === 'drivers') render(); };
  const issue = (e) => {
    const key = `drvError_${e}`;
    return I18N.en.ui[key] ? t(key) : String(e);
  };
  const problemText = (code) => I18N[language].problems?.[code] ?? I18N.en.problems[code] ?? `#${code}`;

  async function load() {
    try {
      inventory = await invoke('driver_inventory');
    } catch (e) {
      loadError = String(e);
    }
    paint();
    // The overview lists device problems too; redraw it through the window.
    if (current === 'overview') window.render();
  }

  /** The graphics vendor whose app has newer drivers, from the display driver. */
  function vendorOf(d) {
    const s = `${d.provider} ${d.device}`.toLowerCase();
    if (s.includes('nvidia')) return ['nvidia', 'NVIDIA'];
    if (s.includes('amd') || s.includes('radeon') || s.includes('advanced micro')) return ['amd', 'AMD'];
    if (s.includes('intel')) return ['intel', 'Intel'];
    return null;
  }

  function problemsCard() {
    const list = inventory.problems;
    if (!list.length) {
      return `<div class="card drv-ok"><span class="dot">${svg(ICONS.ok)}</span><b>${esc(t('drvNoProblems'))}</b></div>`;
    }
    const rows = list.map((p) => `<div class="item"><div><div class="name">${esc(p.name)}</div>` +
      `<div class="sub">${esc(problemText(p.code))}${p.class ? ` · ${esc(p.class)}` : ''}</div></div>` +
      `<span class="tag warn">#${p.code}</span><span></span></div>`).join('');
    return `<div class="card drv-problems"><div class="row-between"><h3>${esc(t('drvProblems'))}</h3>` +
      `<button class="btn small" data-tool="devmgmt">${esc(t('drvOpenDevmgmt'))}</button></div>${rows}` +
      `<p class="note">${esc(t('drvProblemsHint'))}</p></div>`;
  }

  function updatesCard() {
    let body;
    if (searching) {
      body = `<div class="drv-wait"><span class="spinner"></span>${esc(t('drvSearching'))}</div>`;
    } else if (searchError) {
      body = `<p class="fd-warning">${esc(t('drvSearchFailed', { error: searchError }))}</p>`;
    } else if (updates && !updates.length) {
      body = `<div class="drv-ok"><span class="dot">${svg(ICONS.ok)}</span><b>${esc(t('drvNoUpdates'))}</b></div>`;
    } else if (updates) {
      body = updates.map((u) => `<label class="drv-update"><input type="checkbox" data-drv-pick="${esc(u.id)}"` +
        `${picked.has(u.id) ? ' checked' : ''}${installing ? ' disabled' : ''}>` +
        `<div><b>${esc(u.title)}</b><span>${esc([u.model, u.provider, u.date].filter(Boolean).join(' · '))}</span></div>` +
        `<span class="size">${esc(u.size ? bytes(u.size) : '')}</span></label>`).join('');
    } else {
      body = '';
    }
    const canInstall = updates?.length && !installing && !searching;
    const actions = updates?.length
      ? (admin
        ? `<button class="btn primary" data-drv="install"${canInstall && picked.size ? '' : ' disabled'}>${esc(installing ? t('drvInstalling') : tn('drvInstall', picked.size))}</button>`
        : '')
      : '';
    return `<div class="card"><div class="row-between"><h3>${esc(t('drvUpdates'))}</h3>` +
      `<div class="toolbar" style="margin:0">${actions}<button class="btn" data-drv="search"${searching || installing ? ' disabled' : ''}>` +
      `${esc(t(updates || searchError ? 'drvSearchAgain' : 'drvSearch'))}</button></div></div>` +
      `<p class="note" style="margin-top:0">${esc(t('drvUpdatesIntro'))}</p>` +
      (updates?.length ? adminBanner(t('drvInstallAdmin')) : '') + body + resultCard() + '</div>';
  }

  function resultCard() {
    if (!outcome) return '';
    const label = { installed: 'drvResultInstalled', failed: 'drvResultFailed', skipped: 'drvResultSkipped' };
    const rows = outcome.results.map((r) => `<div class="drv-result ${r.result}"><span>${esc(r.title)}</span><b>${esc(t(label[r.result] || 'drvResultFailed'))}</b></div>`).join('');
    const restore = { created: 'drvRestoreCreated', recent: 'drvRestoreRecent', skipped: 'drvRestoreSkipped' }[outcome.restorePoint];
    return `<div class="drv-outcome"><h4>${esc(t('drvResult'))}</h4>${rows}` +
      (restore ? `<p class="note">${esc(t(restore))}</p>` : '') +
      (outcome.reboot ? `<p class="fd-warning">${esc(t('drvReboot'))}</p>` : '') + '</div>';
  }

  function graphicsCard() {
    const gpus = inventory.drivers.filter((d) => d.class.toUpperCase() === 'DISPLAY');
    if (!gpus.length) return '';
    const rows = gpus.map((d) => {
      const vendor = vendorOf(d);
      const button = vendor ? `<button class="btn small" data-drv-vendor="${vendor[0]}">${esc(t('drvVendorApp', { name: vendor[1] }))}</button>` : '<span></span>';
      const logo = typeof brandLogo === 'function' ? brandLogo(brandOf(d.device, d.provider), 'small') : '';
      return `<div class="item"><div><div class="name">${logo}${esc(d.device)}</div>` +
        `<div class="sub">${esc(t('drvVersion'))} ${esc(d.version)} · ${esc(d.date)} · ${esc(d.provider)}</div></div><span></span>${button}</div>`;
    }).join('');
    return `<div class="card"><h3>${esc(t('drvGraphics'))}</h3>${rows}<p class="note">${esc(t('drvGraphicsNote'))}</p></div>`;
  }

  function shownDrivers() {
    const q = query.trim().toLowerCase();
    return inventory.drivers
      .filter((d) => showAll || !d.inbox)
      .filter((d) => !q || `${d.device} ${d.provider} ${d.class} ${d.version}`.toLowerCase().includes(q));
  }

  function listBody(shown) {
    const groups = new Map();
    for (const d of shown) {
      if (!groups.has(d.class)) groups.set(d.class, []);
      groups.get(d.class).push(d);
    }
    return [...groups].map(([cls, list]) => `<h4 class="drv-class">${esc(cls || '—')}</h4>` +
      list.map((d) => `<div class="drv-row"><span class="name" title="${esc(d.device)}">${esc(d.device)}</span>` +
        `<span>${esc(d.provider)}</span><span class="mono">${esc(d.version)}</span><span>${esc(d.date)}</span></div>`).join('')).join('');
  }

  function listCard() {
    const shown = shownDrivers();
    const body = listBody(shown);
    return `<div class="card"><div class="row-between"><h3>${esc(t('drvInstalled'))}</h3><span class="note" id="drv-count">${esc(tn('drvCount', shown.length))}</span></div>` +
      `<div class="toolbar"><input id="drv-search" class="search" type="search" placeholder="${esc(t('drvSearchList'))}" value="${esc(query)}">` +
      `<div class="seg"><button class="${showAll ? '' : 'on'}" data-drv="third">${esc(t('drvThirdParty'))}</button>` +
      `<button class="${showAll ? 'on' : ''}" data-drv="all">${esc(t('drvAll'))}</button></div></div>` +
      `<div class="drv-head"><span>${esc(t('drvTitle'))}</span><span>${esc(t('drvProvider'))}</span><span>${esc(t('drvVersion'))}</span><span>${esc(t('drvDate'))}</span></div>` +
      `<div id="drv-list">${body}</div></div>`;
  }

  function backupCard() {
    return `<div class="card"><div class="row-between"><h3>${esc(t('drvBackup'))}</h3>` +
      `<button class="btn" data-drv="backup"${admin && !backingUp ? '' : ' disabled'}>${esc(t(backingUp ? 'drvBackingUp' : 'drvBackupRun'))}</button></div>` +
      `<p class="note" style="margin-top:0">${esc(t('drvBackupText'))}</p>${adminBanner(t('drvBackupAdmin'))}</div>`;
  }

  function render() {
    if (loadError) { pageEl.innerHTML = `<div class="card fd-error">${esc(loadError)}</div>`; return; }
    if (!inventory) { pageEl.innerHTML = `<p class="loading">${esc(t('drvLoading'))}</p>`; return; }
    pageEl.innerHTML = problemsCard() + updatesCard() + graphicsCard() + listCard() + backupCard() +
      `<p class="note">${esc(t('drvHonest'))}</p>`;
    const input = $('drv-search');
    input.addEventListener('input', () => {
      query = input.value;
      const shown = shownDrivers();
      $('drv-list').innerHTML = listBody(shown);
      $('drv-count').textContent = tn('drvCount', shown.length);
    });
  }

  async function search() {
    searching = true; searchError = ''; outcome = null; paint();
    try {
      updates = await invoke('search_driver_updates');
      picked = new Set(updates.map((u) => u.id));
    } catch (e) {
      searchError = String(e);
    }
    searching = false; paint();
  }

  async function install(skipRestorePoint = false) {
    const ids = [...picked];
    if (!skipRestorePoint && !(await confirmBox(tn('drvConfirm', ids.length), t('drvConfirmText'), tn('drvInstall', ids.length), false))) return;
    installing = true; outcome = null; paint();
    try {
      outcome = await invoke('install_driver_updates', { ids, skipRestorePoint });
    } catch (e) {
      const text = String(e);
      installing = false; paint();
      if (text.startsWith('restorePoint:')) {
        if (await confirmBox(t('drvNoRestorePoint'), t('drvNoRestorePointText', { error: text.slice(13) }), t('drvInstallAnyway'), true)) {
          return install(true);
        }
      } else {
        toast(issue(text));
      }
      return;
    }
    installing = false;
    const done = new Set(outcome.results.filter((r) => r.result === 'installed').map((r) => r.id));
    updates = updates.filter((u) => !done.has(u.id));
    picked = new Set([...picked].filter((id) => !done.has(id)));
    paint();
    load();
  }

  async function backup() {
    backingUp = true; paint();
    try {
      const path = await invoke('backup_drivers');
      toast(t('drvBackedUp', { path }));
    } catch (e) {
      toast(issue(e));
    }
    backingUp = false; paint();
  }

  pageEl.addEventListener('click', (e) => {
    if (current !== 'drivers') return;
    const el = e.target.closest('[data-drv], [data-drv-vendor]');
    if (!el || el.disabled) return;
    if (el.dataset.drvVendor) return attempt(() => invoke('open_vendor_drivers', { vendor: el.dataset.drvVendor }));
    const act = el.dataset.drv;
    if (act === 'search') search();
    else if (act === 'install') install();
    else if (act === 'backup') backup();
    else if (act === 'third' || act === 'all') { showAll = act === 'all'; render(); }
  });

  pageEl.addEventListener('change', (e) => {
    const box = e.target.closest('[data-drv-pick]');
    if (!box) return;
    box.checked ? picked.add(box.dataset.drvPick) : picked.delete(box.dataset.drvPick);
    render();
  });

  PAGES.drivers = { render };
  load();

  return { problemCount: () => inventory?.problems.filter((p) => p.code !== 22 && p.code !== 45).length || 0 };
})();
