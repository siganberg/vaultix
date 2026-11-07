let S = null;
const app = $('#app');
const F = { server: '', username: '', password: '', code: '', localDir: '', remoteDir: '/', autostart: true };
let step = 'signin'; // signin → code → folders → done
let busy = false, error = '', codeRejected = false;

function hero(mark, title, text) {
  return h('div', { class: 'hero fade-in' }, mark === 'ok' ? h('div', { class: 'ok' }, icon('check')) : h('img', { class: 'mark', src: 'icon.svg', alt: '' }), h('h1', {}, title), h('p', {}, text));
}
const field = (label, input, hint) => h('label', { class: 'field' }, h('span', {}, label), input, hint && h('small', {}, hint));
const errorLine = () => h('div', { class: 'err' }, error ? [icon('alert'), h('span', {}, error)] : '');
function bind(key, attrs = {}) {
  return h('input', { class: 'input', value: F[key], spellcheck: 'false', autocomplete: 'off', ...attrs, oninput: (e) => { F[key] = e.target.value; refreshButtons(); } });
}
let primaryBtn = null;
function refreshButtons() {
  if (!primaryBtn) return;
  const ok = step === 'signin' ? F.server.trim() && F.username.trim() && F.password : step === 'code' ? F.code.trim().length >= 6 : step === 'folders' ? F.localDir.trim() : true;
  primaryBtn.disabled = busy || !ok;
}

function signIn(e) {
  e && e.preventDefault();
  if (busy) return;
  busy = true; error = ''; codeRejected = false;
  render();
  engine({ cmd: 'login', server: F.server, username: F.username, password: F.password, code: step === 'code' ? F.code.trim() : null });
}

function render() {
  primaryBtn = null;
  const dev = thisDevice(S);
  if (step === 'signin') {
    primaryBtn = h('button', { class: 'btn primary big', type: 'submit' }, busy ? [icon('sync', 'spin'), 'Signing in…'] : 'Sign In');
    const server = bind('server', { placeholder: 'vaultix.example.com', autofocus: true });
    mount(app, h('form', { onsubmit: signIn },
      hero('logo', 'Sign in to Vaultix', `Connect ${dev} to your drive.`),
      field('Server', server),
      field('Username', bind('username', { autocapitalize: 'off' })),
      field('Password', bind('password', { type: 'password' }), 'An app password from Settings works too, and skips 2FA.'),
      errorLine(),
      h('div', { class: 'grow' }),
      primaryBtn));
    (F.server ? (F.username ? app.querySelector('input[type=password]') : app.querySelectorAll('input')[1]) : server).focus();
  } else if (step === 'code') {
    primaryBtn = h('button', { class: 'btn primary', type: 'submit' }, busy ? [icon('sync', 'spin'), 'Verifying…'] : 'Verify');
    const code = bind('code', { class: 'input code', placeholder: '000000', inputmode: 'numeric', maxlength: '16' });
    code.addEventListener('input', () => { if (/^\d{6}$/.test(F.code.trim())) signIn(); });
    mount(app, h('form', { onsubmit: signIn },
      hero('logo', 'Two-factor authentication', 'Enter the 6-digit code from your authenticator app, or one of your recovery codes.'),
      code,
      h('div', { style: 'height:12px' }),
      codeRejected ? h('div', { class: 'err' }, icon('alert'), "That code didn't work. Try the next one.") : errorLine(),
      h('div', { class: 'grow' }),
      h('div', { class: 'actions' }, h('button', { class: 'btn', type: 'button', onclick: () => { step = 'signin'; F.code = ''; error = ''; busy = false; render(); } }, 'Back'), primaryBtn)));
    code.focus();
  } else if (step === 'folders') {
    primaryBtn = h('button', { class: 'btn primary big', onclick: finish }, 'Start Syncing');
    const remote = bind('remoteDir', { placeholder: '/' });
    mount(app, 
      hero('logo', 'Choose what to sync', `Signed in as ${F.username.trim().toLowerCase()}. Pick where your files live on ${dev}.`),
      h('div', { class: 'field' }, h('span', {}, 'On ' + dev),
        h('div', { class: 'pathbox' }, h('div', { class: 'fi' }, FileIcons.el('x', true)),
          h('div', { class: 'p' }, h('b', {}, folderLabel(F.localDir)), h('small', { title: F.localDir }, '‎' + F.localDir)),
          h('button', { class: 'btn', onclick: choose }, 'Change…'))),
      field('Folder on the server', remote, '“/” syncs your whole drive. Use e.g. /Projects to sync just one folder.'),
      h('label', { class: 'check' }, h('span', { class: 'switch' }, h('input', { type: 'checkbox', checked: F.autostart, onchange: (e) => { F.autostart = e.target.checked; } }), h('i')),
        h('span', {}, 'Start Vaultix when I sign in to ' + dev)),
      errorLine(),
      h('div', { class: 'grow' }),
      primaryBtn);
  } else {
    mount(app, 
      hero('ok', "You're all set", S.platform === 'windows'
        ? 'Vaultix lives in the notification area of your taskbar. Click its icon any time to see what’s syncing.'
        : 'Vaultix lives in your system tray. Open it from there any time to see what’s syncing.'),
      h('div', { class: 'pathbox' }, h('div', { class: 'fi' }, FileIcons.el('x', true)), h('div', { class: 'p' }, h('b', {}, folderLabel(F.localDir)), h('small', {}, '‎' + F.localDir))),
      h('div', { class: 'grow' }),
      h('div', { class: 'actions' },
        h('button', { class: 'btn', onclick: () => invoke('open_target', { what: 'folder' }) }, icon('folder'), 'Open Folder'),
        h('button', { class: 'btn primary', onclick: () => invoke('close_window') }, 'Done')));
  }
  refreshButtons();
}

async function choose() {
  const p = await invoke('pick_folder', { start: F.localDir });
  if (p) { F.localDir = p; render(); }
}

function finish() {
  const remote = '/' + F.remoteDir.trim().replace(/^\/+|\/+$/g, '');
  engine({ cmd: 'configure', local_dir: F.localDir.trim(), remote_dir: remote });
  invoke('set_pref', { key: 'autostart', value: F.autostart }).catch(() => {});
  step = 'done';
  render();
}

(async () => {
  S = await invoke('get_snapshot');
  F.server = S.state.server || '';
  F.username = S.state.username || '';
  F.localDir = S.state.local_dir || S.default_dir;
  F.remoteDir = '/' + (S.state.remote_dir || '');
  render();
  listen('snapshot', (s) => { S = s; });
  listen('login_result', (r) => {
    busy = false;
    if (r.ok) { error = ''; step = 'folders'; F.code = ''; }
    else if (r.need_code) { codeRejected = step === 'code'; step = 'code'; error = ''; F.code = ''; }
    else { error = r.error || 'Sign-in failed.'; }
    render();
  });
  listen('engine_error', (e) => { if (step === 'done' || step === 'folders') { error = e.message; step = 'folders'; render(); } });
})();
