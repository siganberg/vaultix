let V = null;
let editing = false;       // settings fields are being edited: don't re-render under the user
let draft = null;          // { data_dir, port, lan } while editing / during first-run
let busy = false, formError = '', copied = '';
const app = $('#app');

const STATUS = { running: 'Running', starting: 'Starting…', stopped: 'Stopped', error: 'Needs attention' };
const sw = (checked, onchange) => h('span', { class: 'switch' }, h('input', { type: 'checkbox', checked, onchange: (e) => onchange(e.target.checked) }), h('i'));
const row = (title, sub, control) => h('div', { class: 'row' }, h('div', { class: 't' }, h('span', {}, title), sub && h('small', {}, sub)), control);
const device = () => (V.platform === 'windows' ? 'this PC' : V.platform === 'macos' ? 'this Mac' : 'this computer');

function folderBox(path, onChange) {
  return h('div', { class: 'pathbox' }, h('div', { class: 'fi' }, folderIcon()),
    h('div', { class: 'p' }, h('b', {}, folderLabel(path)), h('small', { title: path }, '‎' + path)),
    onChange ? h('button', { class: 'btn', onclick: onChange }, 'Change…') : h('button', { class: 'btn', onclick: () => invoke('open_target', { what: 'data' }) }, 'Show'));
}
function folderIcon() {
  const s = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  s.setAttribute('viewBox', '0 0 48 48');
  s.innerHTML = '<path d="M4 12.5A4.5 4.5 0 0 1 8.5 8h9.6a4.5 4.5 0 0 1 3.2 1.3l2.4 2.4a4.5 4.5 0 0 0 3.2 1.3h12.6a4.5 4.5 0 0 1 4.5 4.5v19A4.5 4.5 0 0 1 39.5 41h-31A4.5 4.5 0 0 1 4 36.5z" fill="#5f55cc"/><path d="M4 21a4 4 0 0 1 4-4h32a4 4 0 0 1 4 4v15.5a4.5 4.5 0 0 1-4.5 4.5h-31A4.5 4.5 0 0 1 4 36.5z" fill="#7468f8"/>';
  return s;
}

async function chooseFolder() {
  const p = await invoke('pick_folder', { start: draft.data_dir });
  if (p) { draft.data_dir = p; render(); }
}
async function apply() {
  const port = parseInt(draft.port, 10);
  if (!(port >= 1 && port <= 65535)) { formError = 'Choose a port between 1 and 65535 (8085 is a good default).'; return render(); }
  busy = true; formError = ''; render();
  try {
    const firstRun = !V.configured, autostart = draft.autostart;
    await invoke('apply_settings', { settings: { data_dir: draft.data_dir, port, lan: !!draft.lan } });
    if (firstRun) await invoke('set_autostart', { enabled: !!autostart }).catch(() => {});
    editing = false; draft = null;
  } catch (e) { formError = String(e); }
  busy = false;
  V = await invoke('get_state');
  render();
}
function copy(text) {
  const done = () => { copied = text; render(); setTimeout(() => { copied = ''; render(); }, 1500); };
  (navigator.clipboard ? navigator.clipboard.writeText(text) : Promise.reject()).then(done).catch(() => {
    const t = h('textarea', { style: 'position:fixed;opacity:0' }); t.value = text; document.body.append(t); t.select();
    try { document.execCommand('copy'); } catch {}
    t.remove(); done();
  });
}

/* ---------- first run ---------- */
function setupView() {
  if (!draft) draft = { data_dir: V.data_dir, port: V.port, lan: true, autostart: true };
  mount(app,
    h('div', { class: 'welcome fade-in' }, h('img', { src: 'icon.svg', alt: '' }), h('h1', {}, 'Set up Vaultix Server'),
      h('p', {}, `Turn ${device()} into your own private cloud drive. Your files stay on this computer.`)),
    h('div', { class: 'field' }, h('span', {}, 'Where should Vaultix keep everyone’s files?'), folderBox(draft.data_dir, chooseFolder),
      h('small', {}, 'Pick a drive with plenty of free space. Files are stored as normal files you can back up.')),
    h('div', { class: 'group' },
      row('Let other devices on my network connect', 'Phones, laptops and the Vaultix apps at home', sw(draft.lan, (v) => { draft.lan = v; })),
      row(`Start automatically when I sign in to ${device()}`, 'So your drive comes back after a restart', sw(draft.autostart, (v) => { draft.autostart = v; }))),
    h('div', { class: 'err' }, formError ? [icon('alert'), h('span', {}, formError)] : ''),
    h('div', { class: 'grow' }),
    h('button', { class: 'btn primary big', disabled: busy, onclick: apply }, busy ? [icon('sync', 'spin'), 'Starting…'] : 'Start Server'));
}

/* ---------- main ---------- */
function mainView() {
  const running = V.status === 'running', active = running || V.status === 'starting';
  const kids = [
    h('div', { class: 'hero' }, h('img', { src: 'icon.svg', alt: '' }),
      h('div', {}, h('h1', {}, 'Vaultix Server'), h('div', { class: 'pill ' + V.status }, h('i'), STATUS[V.status] || V.status)),
      h('button', { class: 'btn', disabled: V.status === 'starting', onclick: () => invoke('server', { action: active ? 'stop' : 'start' }) }, active ? 'Stop' : 'Start')),
  ];
  if (V.status === 'error') {
    kids.push(h('div', { class: 'callout bad' }, h('div', { class: 't' }, h('b', {}, 'The server couldn’t start'), h('small', {}, V.error)),
      h('button', { class: 'btn', onclick: () => invoke('server', { action: 'restart' }) }, 'Try Again')));
  } else if (running && V.users === 0) {
    kids.push(h('div', { class: 'callout fade-in' }, h('div', { class: 't' }, h('b', {}, 'Next: create your account'), h('small', {}, 'Open Vaultix in your browser and choose a username and password. That first account is the admin.')),
      h('button', { class: 'btn primary', onclick: () => invoke('open_target', { what: 'web' }) }, 'Open Vaultix')));
  } else if (running) {
    kids.push(h('div', { class: 'callout' }, h('div', { class: 't' }, h('b', {}, 'Your drive is running'), h('small', {}, `${V.users} account${V.users === 1 ? '' : 's'}. Keep ${device()} on and awake so other devices can reach it.`)),
      h('button', { class: 'btn primary', onclick: () => invoke('open_target', { what: 'web' }) }, 'Open Vaultix')));
  } else if (V.status === 'stopped') {
    kids.push(h('div', { class: 'callout' }, h('div', { class: 't' }, h('b', {}, 'The server is stopped'), h('small', {}, 'Other devices can’t reach your files until you start it again.'))));
  }

  if (running) {
    const addr = (url, note) => h('div', { class: 'addr' }, h('code', { title: url }, url), note && h('small', {}, note),
      h('button', { class: 'btn', onclick: () => copy(url) }, copied === url ? [icon('check'), 'Copied'] : 'Copy'));
    kids.push(h('div', { class: 'label' }, V.lan ? 'CONNECT OTHER DEVICES' : 'ADDRESS'),
      h('div', { class: 'group' },
        V.lan && V.lan_url ? addr(V.lan_url) : '',
        V.lan && V.host_url && !V.lan_url ? addr(V.host_url) : '',
        !V.lan || (!V.lan_url && !V.host_url) ? addr(V.local_url, 'this computer only') : ''),
      V.lan ? h('p', { class: 'note' }, 'Type this address into a browser or the Vaultix app on any device on your home network. ',
        h('a', { href: '#', style: 'color:var(--accent)', onclick: (e) => { e.preventDefault(); invoke('open_target', { what: 'apps' }); } }, 'Get the apps'), ' · ',
        h('a', { href: '#', style: 'color:var(--accent)', onclick: (e) => { e.preventDefault(); invoke('open_target', { what: 'help' }); } }, 'Use it away from home')) : '');
  }

  // Settings
  const d = draft || { data_dir: V.data_dir, port: V.port, lan: V.lan };
  const startEdit = () => { if (!draft) draft = { data_dir: V.data_dir, port: V.port, lan: V.lan }; editing = true; };
  const changed = draft && (draft.data_dir !== V.data_dir || String(draft.port) !== String(V.port) || !!draft.lan !== V.lan);
  kids.push(h('div', { class: 'label' }, 'SETTINGS'),
    h('div', { class: 'group' },
      h('div', { class: 'row' }, h('div', { class: 'pathbox', style: 'flex:1;border:0;padding:0;background:none' }, h('div', { class: 'fi' }, folderIcon()),
        h('div', { class: 'p' }, h('b', {}, 'Files location'), h('small', { title: d.data_dir }, '‎' + d.data_dir))),
        h('button', { class: 'btn', onclick: () => invoke('open_target', { what: 'files' }) }, 'Open'),
        h('button', { class: 'btn', onclick: async () => { startEdit(); await chooseFolder(); } }, 'Change…')),
      row('Port', null, h('input', { class: 'input port', value: d.port, inputmode: 'numeric', maxlength: '5', onfocus: startEdit, oninput: (e) => { startEdit(); draft.port = e.target.value.replace(/\D/g, ''); e.target.value = draft.port; syncApply(); } })),
      row('Let other devices on my network connect', null, sw(d.lan, (v) => { startEdit(); draft.lan = v; render(); })),
      row(`Start when I sign in to ${device()}`, null, sw(V.autostart, (v) => invoke('set_autostart', { enabled: v })))),
    changed && draft.data_dir !== V.data_dir ? h('p', { class: 'note' }, 'Vaultix will use the new folder from now on. Files in the old folder stay there; move them yourself first if you want to keep them.') : '',
    h('div', { class: 'err' }, formError ? [icon('alert'), h('span', {}, formError)] : ''),
    h('div', { id: 'applybar', style: 'display:' + (changed ? 'flex' : 'none') + ';gap:8px;justify-content:flex-end;margin-bottom:6px' },
      h('button', { class: 'btn', onclick: () => { draft = null; editing = false; formError = ''; render(); } }, 'Cancel'),
      h('button', { class: 'btn primary', disabled: busy, onclick: apply }, busy ? 'Restarting…' : 'Save & Restart')),
    h('div', { class: 'foot' }, h('span', {}, 'Version ' + V.version), h('span', { class: 'sp' }),
      h('button', { class: 'btn ghost', onclick: () => invoke('open_target', { what: 'log' }) }, 'View Log'),
      h('button', { class: 'btn ghost', onclick: () => invoke('quit') }, 'Quit')));
  mount(app, kids);
}
function syncApply() {
  const bar = $('#applybar');
  if (bar && draft) bar.style.display = (draft.data_dir !== V.data_dir || String(draft.port) !== String(V.port) || !!draft.lan !== V.lan) ? 'flex' : 'none';
}

function render() {
  if (!V) return;
  (V.configured ? mainView : setupView)();
}

addEventListener('keydown', (e) => { if (e.key === 'Escape') invoke('hide_window'); });
(async () => {
  V = await invoke('get_state');
  render();
  listen('state', (v) => {
    const focusedInput = document.activeElement && document.activeElement.classList.contains('port');
    const same = JSON.stringify(v) === JSON.stringify(V);
    V = v;
    if (!same && !focusedInput) render();
  });
})();
