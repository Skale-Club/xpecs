'use strict';

// Explicit file deletion. This module never participates in Cleaner scans.
window.XcareForceDelete = (() => {
  let paths = [], preview = null, result = null, error = '', working = false, draft = '';
  const invokeNative = (...args) => window.__TAURI__.core.invoke(...args);
  const paint = () => { if (current === 'forceDelete') render(); };
  const issue = (message) => {
    const key = `fdError_${message}`;
    return I18N.en.ui[key] ? t(key) : String(message);
  };
  const pathList = (items) => `<ul class="fd-paths">${items.map((p) => `<li title="${esc(p)}">${esc(p)}</li>`).join('')}</ul>`;

  function renderPage(target) {
    const disabled = working ? ' disabled' : '';
    let details = '';
    if (preview) {
      const processes = preview.blockers.map((p) => `<div class="fd-process"><div><b>${esc(p.name || t('fdUnknownApp'))}</b>` +
        `<span class="note">PID ${p.pid}</span></div><span class="fd-status ${p.canTerminate ? 'warn' : ''}">${esc(t(p.canTerminate ? 'fdCanClose' : 'fdProtectedApp'))}</span></div>`).join('');
      details = `<section class="card fd-review" aria-label="${esc(t('fdReview'))}"><div class="row-between"><h3>${esc(t('fdReview'))}</h3>` +
        `<span class="fd-status">${esc(t('fdReady'))}</span></div>` + pathList(preview.paths) +
        `<div class="fd-facts"><span><b>${num(preview.files)}</b>${esc(t('fdFiles'))}</span><span><b>${num(preview.folders)}</b>${esc(t('fdFolders'))}</span><span><b>${esc(bytes(preview.bytes))}</b>${esc(t('fdSize'))}</span></div>` +
        `<h4>${esc(t('fdBlockingApps'))}</h4>` + (preview.blockerError ? `<p class="fd-warning">${esc(t('fdDetectionFailed'))} ${esc(preview.blockerError)}</p>` :
          processes || `<p class="note">${esc(t('fdNoBlockers'))}</p>`) +
        `<div class="fd-actions"><button class="btn danger" data-fd="delete"${disabled}>${esc(t('fdDelete'))}</button>` +
        `<button class="btn" data-fd="terminate"${working || preview.blockerError || !preview.blockers.length || preview.blockers.some((p) => !p.canTerminate) ? ' disabled' : ''}>${esc(t('fdTerminate'))}</button>` +
        `<button class="btn" data-fd="reboot"${working || !admin ? ' disabled' : ''}>${esc(t('fdReboot'))}</button></div>` +
        (!admin ? `<p class="note">${esc(t('fdAdminNote'))} <button class="btn small" data-action="admin"${disabled}>${esc(t('restartAdmin'))}</button></p>` : '') +
        `<p class="note">${esc(t('fdSnapshotNote'))}</p></section>`;
    }
    if (result) {
      details += `<section class="card fd-result" role="status"><h3>${esc(t('fdResult'))}</h3>` +
        `<p>${esc(t('fdResultCounts', { deleted: num(result.deleted), scheduled: num(result.scheduled), failed: num(result.failed), closed: num(result.closed) }))}</p>` +
        (result.scheduled ? `<p class="fd-warning">${esc(t('fdScheduledNote'))}</p>` : '') +
        (result.failed ? `<details open><summary>${esc(t('fdFailures'))}</summary><ul class="fd-errors">${result.errors.map((e) => `<li><b>${esc(e.path)}</b><span>${esc(issue(e.error))}</span></li>`).join('')}</ul>` +
          (result.failed > result.errors.length ? `<p class="note">${esc(t('fdErrorLimit'))}</p>` : '') + '</details>' : '') + `</section>`;
    }
    target.innerHTML = `<div class="fd-intro"><span class="fd-symbol">${svg(ICONS.forceDelete)}</span><div><h2>${esc(t('fdHeading'))}</h2><p>${esc(t('fdIntro'))}</p></div></div>` +
      `<section class="card fd-selection"><div class="row-between"><h3>${esc(t('fdSelection'))}</h3><span class="note">${esc(t('fdLimit'))}</span></div>` +
      `<div class="fd-drop" id="fd-drop" aria-busy="${working}"><b>${esc(t(working ? 'fdWorking' : 'fdDrop'))}</b><span>${esc(t('fdDropHint'))}</span>` +
      `<div class="toolbar"><button class="btn primary" data-fd="files"${disabled}>${esc(t('fdChooseFiles'))}</button><button class="btn" data-fd="folder"${disabled}>${esc(t('fdChooseFolder'))}</button></div></div>` +
      `<label class="note" for="fd-path">${esc(t('fdPathLabel'))}</label><div class="toolbar fd-path-input"><input class="field" id="fd-path" value="${esc(draft)}" placeholder="C:\\Users\\…" spellcheck="false"${disabled}>` +
      `<button class="btn" data-fd="add"${disabled}>${esc(t('fdAddPath'))}</button></div>` +
      (paths.length ? `<div class="fd-picked">${paths.map((p, i) => `<div><span title="${esc(p)}">${esc(p)}</span><button class="btn small" data-fd="remove" data-index="${i}" aria-label="${esc(t('fdRemove', { path: p }))}"${disabled}>${esc(t('fdRemoveShort'))}</button></div>`).join('')}</div>` : '') +
      `<div class="fd-selection-actions"><button class="btn" data-fd="clear"${working || !paths.length ? ' disabled' : ''}>${esc(t('fdClear'))}</button>` +
      `<button class="btn primary" data-fd="inspect"${working || !paths.length ? ' disabled' : ''}>${esc(t(preview ? 'fdRefresh' : 'fdInspect'))}</button></div></section>` +
      (error ? `<div class="card fd-error" role="alert">${esc(issue(error))}</div>` : '') + details +
      `<p class="note fd-footnote">${esc(t('fdProtectionNote'))}</p>`;
    const input = document.getElementById('fd-path');
    input.addEventListener('input', () => { draft = input.value; });
    input.addEventListener('keydown', (event) => { if (event.key === 'Enter' && !working) { event.preventDefault(); addPaths([draft]); } });
  }

  function addPaths(incoming) {
    if (working) return;
    const nonempty = incoming.map((p) => p.trim()).filter(Boolean);
    if (!nonempty.length) return;
    const combined = [...paths];
    for (const p of nonempty) if (!combined.some((old) => old.toLowerCase() === p.toLowerCase())) combined.push(p);
    if (combined.length > 100) { error = 'invalidSelection'; paint(); return; }
    paths = combined; preview = null; result = null; error = ''; draft = ''; paint();
  }

  async function act(action, index) {
    if (working) return;
    if (action === 'add') { addPaths([draft]); return; }
    if (action === 'clear' || action === 'remove') {
      paths = action === 'clear' ? [] : paths.filter((_, i) => i !== index);
      preview = null; result = null; error = ''; paint(); return;
    }
    if (['delete', 'terminate', 'reboot'].includes(action)) {
      if (!preview) return;
      const reviewed = preview;
      const processText = action === 'terminate' ? '\n\n' + t('fdConfirmApps') + '\n' + reviewed.blockers.map((p) => `${p.name} (PID ${p.pid})`).join('\n') : '';
      const text = t(action === 'terminate' ? 'fdConfirmTerminate' : action === 'reboot' ? 'fdConfirmReboot' : 'fdConfirmDelete') + '\n\n' + reviewed.paths.join('\n') + processText;
      // Lock the selection while the confirmation is visible too.
      working = true; paint();
      const confirmation = confirmBox(t('forceDelete'), text, t(action === 'terminate' ? 'fdTerminate' : action === 'reboot' ? 'fdReboot' : 'fdDelete'), true);
      document.getElementById('modal-cancel').focus();
      const confirmed = await confirmation;
      if (!confirmed) { working = false; paint(); return; }
      try {
        error = ''; result = await invokeNative('execute_force_delete', { token: reviewed.token, mode: action });
      } catch (e) { error = String(e); }
      finally { preview = null; working = false; paint(); }
      return;
    }
    working = true; error = ''; paint();
    try {
      if (action === 'files' || action === 'folder') {
        const selectedPaths = await invokeNative('pick_force_delete', { folder: action === 'folder' });
        working = false;
        addPaths(selectedPaths);
      } else if (action === 'inspect') {
        preview = null; result = null;
        preview = await invokeNative('prepare_force_delete', { paths: [...paths] });
      }
    } catch (e) { error = String(e); }
    finally { working = false; paint(); }
  }

  // Tauri owns OS drag/drop; dropped paths are only selected, never deleted.
  window.__TAURI__.webview.getCurrentWebview().onDragDropEvent(({ payload }) => {
    if (current !== 'forceDelete' || working) return;
    if (payload.type === 'drop') addPaths(payload.paths);
    document.getElementById('fd-drop')?.classList.toggle('over', payload.type === 'over' || payload.type === 'enter');
  }).catch(() => {});

  return { render: renderPage, act };
})();
