let S = null;
let menuOpen = false;
const app = $('#app');

const VERB = { uploaded: 'Uploaded', downloaded: 'Downloaded', deleted_local: 'Removed from this device', deleted_remote: 'Deleted from server', conflict: 'Conflict – kept both copies' };
const BADGE = { uploaded: ['up', 'up'], downloaded: ['down', 'down'], conflict: ['conflict', 'warn'], deleted_local: ['del', 'trash'], deleted_remote: ['del', 'trash'] };

function statusInfo() {
  const st = S.status || {}, phase = st.phase || 'starting', t = S.transfer;
  if (S.state.paused) return ['paused', 'pause', 'Paused'];
  switch (phase) {
    case 'idle': return ['ok', 'check', 'Up to date'];
    case 'syncing': return ['sync', 'sync', t && t.total > 0 ? `Syncing ${t.index} of ${t.total}…` : 'Checking for changes…'];
    case 'offline': return ['bad', 'offline', st.line || 'Offline'];
    case 'error': return ['bad', 'alert', st.line || 'Sync problem'];
    case 'signedout': return ['bad', 'alert', 'Signed out'];
    default: return ['sync', 'sync', 'Starting…'];
  }
}

function header() {
  const [cls, ic, text] = statusInfo();
  const configured = S.state.configured;
  return h('div', { class: 'head' },
    h('img', { class: 'logo', src: 'icon.svg', alt: '' }),
    h('div', { class: 'who' }, h('b', {}, 'Vaultix'),
      configured ? h('div', { class: 'status ' + cls }, icon(ic, cls === 'sync' ? 'spin' : ''), h('span', { title: text }, text)) : h('div', { class: 'status' }, 'Not signed in')),
    configured && h('button', { class: 'btn round', title: S.state.paused ? 'Resume syncing' : 'Pause syncing', onclick: () => engine({ cmd: 'pause', paused: !S.state.paused }) }, icon(S.state.paused ? 'play' : 'pause')),
    h('button', { class: 'btn round', title: 'More', onclick: (e) => { e.stopPropagation(); menuOpen = !menuOpen; render(); } }, icon('more')));
}

function menu() {
  const configured = S.state.configured;
  const item = (ic, label, fn) => h('button', { onclick: () => { menuOpen = false; render(); fn(); } }, icon(ic), label);
  return h('div', { class: 'menu fade-in', onclick: (e) => e.stopPropagation() },
    configured && item('sync', 'Sync Now', () => engine({ cmd: 'sync_now' })),
    configured && item('globe', 'Open in Browser', () => invoke('open_target', { what: 'web' })),
    configured && h('hr'),
    item('gear', 'Settings…', () => invoke('open_window', { name: 'settings' })),
    item('log', 'Show Log', () => invoke('open_target', { what: 'log' })),
    h('hr'),
    item('power', 'Quit Vaultix', () => invoke('quit')));
}

function transferCard() {
  const t = S.transfer;
  if (!t || (S.status || {}).phase !== 'syncing') return null;
  const frac = t.size > 0 ? Math.min(1, t.bytes / t.size) : null;
  let detail = t.op;
  if (t.size > 0) detail += ` · ${fmtBytes(t.bytes)} of ${fmtBytes(t.size)}`;
  if (t.total > 1) detail += ` · ${t.index} of ${t.total}`;
  return h('div', { class: 'card' },
    h('div', { class: 'xfer' }, h('div', { class: 'op' }, icon(t.op === 'Uploading' ? 'up' : t.op === 'Downloading' ? 'down' : 'sync')),
      h('div', { class: 't' }, h('b', { title: t.path }, baseName(t.path)), h('small', {}, detail))),
    h('div', { class: 'bar' + (frac == null ? ' indet' : '') }, h('i', { style: `width:${(frac || 0) * 100}%` })));
}

function banner() {
  const st = S.status || {};
  if (S.error) return h('div', { class: 'banner' }, icon('alert'), h('span', {}, S.error));
  if (st.phase === 'error') return h('div', { class: 'banner' }, icon('alert'), h('span', {}, st.line));
  if (st.phase === 'offline') return h('div', { class: 'banner warn' }, icon('offline'), h('span', {}, "Can't reach the server. Vaultix will keep trying."));
  return null;
}

function recentList() {
  if (!S.recent.length) {
    return h('div', { class: 'empty' }, h('div', { class: 'big' }, icon('cloud')), h('b', {}, 'Nothing synced yet'), h('span', {}, 'Files you add to your Vaultix folder appear here.'));
  }
  return [h('div', { class: 'sect' }, 'RECENT'),
    h('div', { class: 'list' }, S.recent.map((a) => {
      const [bc, bi] = BADGE[a.op] || ['del', 'sync'];
      const del = a.op.startsWith('deleted');
      const folder = dirName(a.path);
      return h('div', { class: 'item' + (del ? ' del' : ''), title: a.path, onclick: () => invoke('open_target', { what: 'reveal', path: a.path }) },
        h('div', { class: 'ic' }, FileIcons.el(baseName(a.path), !!a.is_dir), h('span', { class: 'badge ' + bc }, icon(bi))),
        h('div', { class: 't' }, h('b', {}, baseName(a.path)),
          h('small', {}, [(VERB[a.op] || a.op).replace('this device', thisDevice(S)), ago(a.at), folder].filter(Boolean).join(' · '))));
    }))];
}

function signedOut(again) {
  return h('div', { class: 'empty' },
    h('div', { class: 'big' }, icon(again ? 'lock' : 'user')),
    h('b', {}, again ? 'Your session ended' : 'Sign in to start syncing'),
    h('span', {}, again ? 'The app password may have been revoked.' : `Connect ${thisDevice(S)} to your Vaultix drive.`),
    h('button', { class: 'btn primary', onclick: () => { if (again) engine({ cmd: 'logout' }); invoke('open_window', { name: 'setup' }); } }, again ? 'Sign In Again…' : 'Sign In…'));
}

function footer() {
  const u = S.usage;
  return h('div', { class: 'foot' },
    h('button', { class: 'btn ghost', onclick: () => invoke('open_target', { what: 'folder' }) }, icon('folder'), 'Folder'),
    h('button', { class: 'btn ghost', onclick: () => invoke('open_target', { what: 'web' }) }, icon('globe'), 'Web'),
    h('div', { class: 'use' }, u ? [h('b', {}, `${fmtBytes(u.used)} in Vaultix`), h('small', {}, `${fmtBytes(u.free)} free on server`)] : ''));
}

function render() {
  if (!S) return;
  const configured = S.state.configured;
  const signedout = (S.status || {}).phase === 'signedout';
  mount(app, 
    header(),
    menuOpen ? menu() : '',
    configured && !signedout ? [transferCard(), banner(), recentList(), footer()] : signedOut(signedout));
}

document.addEventListener('click', () => { if (menuOpen) { menuOpen = false; render(); } });
addEventListener('keydown', (e) => { if (e.key === 'Escape') { if (menuOpen) { menuOpen = false; render(); } else invoke('close_window'); } });
// Relative times ("2 min. ago") stay fresh.
setInterval(render, 30000);

(async () => {
  S = await invoke('get_snapshot');
  render();
  listen('snapshot', (s) => { S = s; render(); });
  listen('popover-shown', () => { menuOpen = false; render(); });
})();
