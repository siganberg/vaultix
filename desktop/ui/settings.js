let S = null;
let tab = 'general';
let confirmOut = false;
let shownTab = null;
const draft = { local: null, remote: null };
const app = $('#app');

const TABS = [['general', 'gear', 'General'], ['account', 'user', 'Account'], ['folders', 'folder', 'Folders'], ['about', 'info', 'About']];

function sw(checked, onchange, disabled) {
  return h('span', { class: 'switch' }, h('input', { type: 'checkbox', checked, disabled, onchange: (e) => onchange(e.target.checked) }), h('i'));
}
const row = (title, sub, control) => h('div', { class: 'row' }, h('div', { class: 't' }, h('span', {}, title), sub && h('small', {}, sub)), control);

function general() {
  const dev = thisDevice(S);
  return [
    h('div', { class: 'group' },
      row('Start Vaultix when I sign in', null, sw(S.autostart, (v) => invoke('set_pref', { key: 'autostart', value: v }).then(load))),
      row('Notify me about conflicts and sync problems', null, sw(S.notifications, (v) => invoke('set_pref', { key: 'notifications', value: v }).then(load)))),
    h('div', { class: 'group' },
      row('Pause syncing', null, sw(!!S.state.paused, (v) => engine({ cmd: 'pause', paused: v }), !S.state.configured)),
      row('Check for changes right away', null, h('button', { class: 'btn', disabled: !S.state.configured || S.state.paused, onclick: () => engine({ cmd: 'sync_now' }) }, 'Sync Now'))),
    h('p', { class: 'note' }, `Changes on ${dev} sync within seconds; changes from other devices within about 15 seconds.`),
  ];
}

function account() {
  if (!S.state.configured) {
    return h('div', { class: 'group' }, h('div', { class: 'acct' }, h('div', { class: 'avatar' }, icon('user')),
      h('div', { class: 't' }, h('b', {}, 'Not signed in'), h('small', {}, 'Sign in to start syncing.')),
      h('button', { class: 'btn primary', onclick: () => invoke('open_window', { name: 'setup' }) }, 'Sign In…')));
  }
  const dev = thisDevice(S);
  return [
    h('div', { class: 'group' },
      h('div', { class: 'acct' }, h('div', { class: 'avatar' }, (S.state.username || '?')[0]),
        h('div', { class: 't' }, h('b', {}, S.state.username), h('small', { title: S.state.server }, S.state.server)),
        h('button', { class: 'btn', onclick: () => invoke('open_target', { what: 'web' }) }, 'Open in Browser'))),
    h('div', { class: 'group' },
      confirmOut
        ? h('div', { class: 'confirm' }, h('p', {}, 'Syncing stops. Nothing is deleted from ' + dev + ' or the server.'),
          h('div', { class: 'btns' }, h('button', { class: 'btn', onclick: () => { confirmOut = false; render(); } }, 'Cancel'),
            h('button', { class: 'btn danger solid', onclick: () => { confirmOut = false; engine({ cmd: 'logout' }); } }, 'Sign Out')))
        : row('Sign out of ' + dev, 'Your files stay in your folder and on the server.', h('button', { class: 'btn danger', onclick: () => { confirmOut = true; render(); } }, 'Sign Out…'))),
    h('p', { class: 'note' }, `To revoke this device's access remotely, use Settings → Devices in the web app.`),
  ];
}

function folders() {
  if (!S.state.configured) return h('p', { class: 'note' }, 'Sign in first to choose folders.');
  const local = draft.local ?? S.state.local_dir;
  const remote = draft.remote ?? '/' + (S.state.remote_dir || '');
  const norm = (r) => r.trim().replace(/^\/+|\/+$/g, '');
  const changed = local !== S.state.local_dir || norm(remote) !== (S.state.remote_dir || '');
  return [
    h('div', { class: 'group' },
      h('div', { class: 'row' }, h('div', { class: 'pathbox', style: 'flex:1;border:0;padding:0;background:none' },
        h('div', { class: 'fi' }, FileIcons.el('x', true)),
        h('div', { class: 'p' }, h('b', {}, folderLabel(local)), h('small', { title: local }, '‎' + local))),
        h('button', { class: 'btn', onclick: () => invoke('open_target', { what: 'folder' }) }, 'Show'),
        h('button', { class: 'btn', onclick: async () => { const p = await invoke('pick_folder', { start: local }); if (p) { draft.local = p; render(); } } }, 'Change…')),
      row('Folder on the server', null, h('input', { class: 'input', style: 'width:200px;text-align:right', value: remote, spellcheck: 'false', oninput: (e) => { draft.remote = e.target.value; refreshApply(); } }))),
    h('p', { class: 'note' }, 'Changing folders starts a fresh sync. Nothing is deleted: files are merged, and anything edited in both places keeps both copies.'),
    h('div', { class: 'apply' },
      h('button', { class: 'btn', id: 'revert', disabled: !changed, onclick: () => { draft.local = draft.remote = null; render(); } }, 'Revert'),
      h('button', { class: 'btn primary', id: 'applyb', disabled: !changed, onclick: () => {
        engine({ cmd: 'configure', local_dir: local, remote_dir: '/' + norm(remote) });
        draft.local = draft.remote = null;
      } }, 'Apply')),
  ];
}
function refreshApply() {
  const local = draft.local ?? S.state.local_dir;
  const changed = local !== S.state.local_dir || (draft.remote ?? '').trim().replace(/^\/+|\/+$/g, '') !== (S.state.remote_dir || '');
  $('#revert').disabled = $('#applyb').disabled = !changed;
}

function about() {
  return h('div', { class: 'about' },
    h('img', { src: 'icon.svg', alt: '' }),
    h('h2', {}, 'Vaultix'),
    h('p', {}, 'Version ' + (S.state.version || '')),
    h('p', { style: 'margin-top:4px' }, 'Your files, on your own server.'),
    h('div', { class: 'btns' },
      h('button', { class: 'btn', onclick: () => invoke('open_target', { what: 'log' }) }, icon('log'), 'Show Log'),
      h('button', { class: 'btn', disabled: !S.state.configured, onclick: () => invoke('open_target', { what: 'web' }) }, icon('globe'), 'Open Web App')));
}

function render() {
  const body = { general, account, folders, about }[tab]();
  mount(app, 
    h('div', { class: 'tabs' }, TABS.map(([k, ic, label]) => h('button', { class: 'tab' + (tab === k ? ' on' : ''), onclick: () => { tab = k; confirmOut = false; render(); } }, icon(ic), label))),
    h('div', { class: 'body' + (shownTab !== tab ? ' fade-in' : '') }, body));
  shownTab = tab;
}

async function load() { S = await invoke('get_snapshot'); render(); }
addEventListener('keydown', (e) => { if (e.key === 'Escape') invoke('close_window'); });
(async () => {
  await load();
  listen('snapshot', (s) => {
    const focused = document.activeElement && document.activeElement.tagName === 'INPUT' && document.activeElement.type !== 'checkbox';
    S = s;
    if (!focused) render();
  });
})();
