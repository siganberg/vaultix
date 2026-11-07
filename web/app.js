'use strict';

/* ================= helpers ================= */

const $ = (s, el = document) => el.querySelector(s);

function h(tag, attrs, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v == null || v === false) continue;
    if (k.startsWith('on')) el.addEventListener(k.slice(2).toLowerCase(), v);
    else if (k === 'class') el.className = v;
    else if (k === 'html') el.innerHTML = v; // only ever used with constant strings
    else if (k === 'value') el.value = v;
    else el.setAttribute(k, v === true ? '' : v);
  }
  for (const c of kids.flat(Infinity)) {
    if (c == null || c === false) continue;
    el.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return el;
}

const P = {
  folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  file: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/>',
  image: '<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-5-5L5 21"/>',
  video: '<rect x="2" y="5" width="15" height="14" rx="2"/><path d="m17 10 5-3v10l-5-3"/>',
  audio: '<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>',
  pdf: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5M9 13h6M9 17h4"/>',
  archive: '<rect x="3" y="4" width="18" height="5" rx="1"/><path d="M5 9v10a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V9M10 13h4"/>',
  code: '<path d="m8 8-5 4 5 4M16 8l5 4-5 4"/>',
  model: '<path d="m12 2 9 5v10l-9 5-9-5V7z"/><path d="m3 7 9 5 9-5M12 12v10"/>',
  upload: '<path d="M12 16V4M7 9l5-5 5 5"/><path d="M20 16v3a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-3"/>',
  download: '<path d="M12 4v12M7 11l5 5 5-5"/><path d="M20 16v3a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-3"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  newfolder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><path d="M12 11v5M9.5 13.5h5"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
  grid: '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>',
  list: '<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/>',
  trash: '<path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/>',
  settings: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>',
  logout: '<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9"/>',
  edit: '<path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 1 1 3 3L7 19l-4 1 1-4z"/>',
  move: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><path d="m12 10 3 3-3 3M8 13h7"/>',
  more: '<circle cx="12" cy="5" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="12" cy="19" r="1"/>',
  x: '<path d="M18 6 6 18M6 6l12 12"/>',
  check: '<path d="m5 12 5 5L20 7"/>',
  left: '<path d="m15 18-6-6 6-6"/>',
  right: '<path d="m9 18 6-6-6-6"/>',
  restore: '<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/>',
  shield: '<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><path d="m9 12 2 2 4-4"/>',
  key: '<circle cx="7.5" cy="15.5" r="4.5"/><path d="m10.7 12.3 9.3-9.3M17 6l3 3M14 9l2 2"/>',
  monitor: '<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>',
  link: '<path d="M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.7 1.7"/><path d="M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7"/>',
  user: '<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>',
  menu: '<path d="M4 6h16M4 12h16M4 18h16"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
  open: '<path d="M15 3h6v6M10 14 21 3M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
};
const icon = (n, cls = '') => {
  const s = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  s.setAttribute('viewBox', '0 0 24 24');
  s.setAttribute('class', 'i ' + cls);
  s.innerHTML = P[n] || P.file;
  return s;
};

const EXT = {
  image: 'jpg jpeg png gif webp avif bmp ico heic svg tif tiff',
  video: 'mp4 webm mov m4v mkv avi',
  audio: 'mp3 wav ogg m4a flac aac opus',
  pdf: 'pdf',
  archive: 'zip tar gz tgz bz2 xz 7z rar dmg iso',
  code: 'js ts tsx jsx json rs py go c h cpp hpp cs java kt swift rb php sh zsh bash yml yaml toml xml html css scss sql ini conf cfg env lua gcode nc ngc tap',
  model: 'stl obj 3mf step stp iges igs f3d dxf svg scad blend fbx gltf glb',
  doc: 'txt md log csv tsv rtf doc docx odt xls xlsx ods ppt pptx',
};
const ext = (n) => { const i = n.lastIndexOf('.'); return i > 0 ? n.slice(i + 1).toLowerCase() : n.toLowerCase(); };
function kind(e) {
  if (e.is_dir) return 'folder';
  const x = ext(e.name);
  for (const k of ['image', 'video', 'audio', 'pdf', 'archive', 'model', 'code', 'doc']) if (EXT[k].split(' ').includes(x)) return k;
  return 'doc';
}
const ficon = (e, big) => h('div', { class: 'ficon' + (big ? ' big' : '') }, FileIcons.el(e.name, e.is_dir, big));
const tint = (e) => FileIcons.type(e.name, e.is_dir).color;

function fmtSize(b) {
  if (b == null) return '';
  if (b < 1024) return b + ' B';
  const u = ['KB', 'MB', 'GB', 'TB'];
  let i = -1;
  do { b /= 1024; i++; } while (b >= 1024 && i < u.length - 1);
  return (b < 10 ? b.toFixed(1) : Math.round(b)) + ' ' + u[i];
}
function fmtDate(ms) {
  if (!ms) return '';
  const d = new Date(ms), now = Date.now(), diff = (now - ms) / 1000;
  if (diff < 60) return 'Just now';
  if (diff < 3600) return Math.floor(diff / 60) + ' min ago';
  if (diff < 86400 && new Date(now).getDate() === d.getDate()) return d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
  if (diff < 7 * 86400) return d.toLocaleDateString([], { weekday: 'short' }) + ' ' + d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
  return d.toLocaleDateString([], { year: d.getFullYear() === new Date().getFullYear() ? undefined : 'numeric', month: 'short', day: 'numeric' });
}
const joinPath = (a, b) => (a ? a + '/' + b : b);
const parentOf = (p) => (p.includes('/') ? p.slice(0, p.lastIndexOf('/')) : '');
const baseName = (p) => p.slice(p.lastIndexOf('/') + 1);
const fileUrl = (p, dl) => '/api/file?path=' + encodeURIComponent(p) + (dl ? '&download=1' : '');

class ApiError extends Error {}
async function api(method, url, body) {
  const r = await fetch(url, {
    method,
    headers: { 'x-vaultix': '1', ...(body ? { 'content-type': 'application/json' } : {}) },
    body: body ? JSON.stringify(body) : undefined,
  });
  let data = null;
  try { data = await r.json(); } catch {}
  if (r.status === 401 && !url.startsWith('/api/auth/')) { showLogin(); throw new ApiError('Signed out'); }
  if (!r.ok) { const e = new ApiError((data && data.error) || r.statusText); e.data = data; e.status = r.status; throw e; }
  return data;
}

/* ================= toasts, menus, modals ================= */

const toastsEl = h('div', { class: 'toasts' });
function toast(msg, opts = {}) {
  const t = h('div', { class: 'toast' + (opts.error ? ' error' : '') }, msg);
  if (opts.action) {
    t.append(' ', h('a', { href: '#', style: 'color:inherit;font-weight:700;margin-left:10px;text-decoration:underline', onclick: (e) => { e.preventDefault(); t.remove(); opts.action.fn(); } }, opts.action.label));
  }
  toastsEl.append(t);
  setTimeout(() => t.remove(), opts.ms || 4000);
}
const fail = (e) => { if (!(e instanceof ApiError && e.message === 'Signed out')) toast(e.message || String(e), { error: true }); };

let menuEl = null;
function closeMenu() { if (menuEl) { menuEl.remove(); menuEl = null; } }
function openMenu(x, y, items) {
  closeMenu();
  menuEl = h('div', { class: 'menu' }, items.filter(Boolean).map((it) =>
    it === '-' ? h('hr') : h('button', { class: it.danger ? 'danger' : '', onclick: () => { closeMenu(); it.fn(); } }, icon(it.icon), it.label)));
  document.body.append(menuEl);
  const r = menuEl.getBoundingClientRect();
  menuEl.style.left = Math.min(x, innerWidth - r.width - 8) + 'px';
  menuEl.style.top = Math.min(y, innerHeight - r.height - 8) + 'px';
}
addEventListener('mousedown', (e) => { if (menuEl && !menuEl.contains(e.target)) closeMenu(); });
addEventListener('blur', closeMenu);

function modal(build) {
  return new Promise((resolve) => {
    const ov = h('div', { class: 'overlay' });
    const close = (v) => { ov.remove(); document.removeEventListener('keydown', esc, true); resolve(v); };
    const esc = (e) => { if (e.key === 'Escape') { e.stopPropagation(); close(null); } };
    document.addEventListener('keydown', esc, true);
    ov.addEventListener('mousedown', (e) => { if (e.target === ov) close(null); });
    const m = h('div', { class: 'modal' });
    build(m, close);
    ov.append(m);
    document.body.append(ov);
    const f = m.querySelector('input'); if (f) { f.focus(); }
  });
}
function prompt2(title, { label, value = '', ok = 'Save', selectStem = false, desc } = {}) {
  return modal((m, close) => {
    const inp = h('input', { class: 'input', value, spellcheck: 'false' });
    const submit = (e) => { e.preventDefault(); const v = inp.value.trim(); if (v) close(v); };
    m.append(h('form', { onsubmit: submit }, h('h2', {}, title), desc && h('p', {}, desc), h('label', { class: 'field' }, label && h('span', {}, label), inp),
      h('div', { class: 'actions' }, h('button', { type: 'button', class: 'btn', onclick: () => close(null) }, 'Cancel'), h('button', { class: 'btn primary' }, ok))));
    setTimeout(() => { inp.focus(); const dot = value.lastIndexOf('.'); if (selectStem && dot > 0) inp.setSelectionRange(0, dot); else inp.select(); });
  });
}
function confirm2(title, desc, { ok = 'Confirm', danger = false } = {}) {
  return modal((m, close) => {
    m.append(h('h2', {}, title), h('p', {}, desc), h('div', { class: 'actions' },
      h('button', { class: 'btn', onclick: () => close(false) }, 'Cancel'),
      h('button', { class: 'btn ' + (danger ? 'danger solid' : 'primary'), onclick: () => close(true) }, ok)));
    setTimeout(() => m.querySelector('.actions .btn:last-child').focus());
  });
}

/* ================= state & routing ================= */

const S = {
  me: null,
  route: 'files',
  path: '',
  entries: [],
  sel: new Set(),
  lastSel: null,
  view: (() => { try { return localStorage.getItem('vx.view') || 'list'; } catch { return 'list'; } })(),
  sort: { key: 'name', dir: 1 },
  query: '',
  usage: null,
  shares: [],
  sharedPaths: new Set(),
};

function parseHash() {
  const raw = location.hash.replace(/^#\/?/, '');
  if (raw.startsWith('trash')) return { route: 'trash' };
  if (raw.startsWith('shared')) return { route: 'shared' };
  if (raw.startsWith('settings')) return { route: 'settings' };
  const p = raw.replace(/^files\/?/, '');
  return { route: 'files', path: p.split('/').filter(Boolean).map(decodeURIComponent).join('/') };
}
function go(path) {
  location.hash = '#/files/' + path.split('/').filter(Boolean).map(encodeURIComponent).join('/');
}
addEventListener('hashchange', () => { if (S.me) route(); });

async function route() {
  const r = parseHash();
  S.route = r.route;
  S.sel.clear();
  closeSidebar();
  if (r.route === 'files') {
    if (S.query) { S.query = ''; const si = $('#search'); if (si) si.value = ''; }
    S.path = r.path;
    await loadFiles();
  } else renderMain();
  for (const n of document.querySelectorAll('.nav[data-r]')) n.classList.toggle('active', n.dataset.r === S.route);
}

async function loadFiles() {
  try {
    const d = await api('GET', '/api/list?path=' + encodeURIComponent(S.path));
    S.entries = d.entries;
  } catch (e) {
    if (e.status === 404 && S.path) { toast('Folder not found', { error: true }); go(parentOf(S.path)); return; }
    fail(e); S.entries = [];
  }
  renderMain();
}
const refresh = () => (S.query ? doSearch(S.query) : loadFiles());

/* ================= login ================= */

const root = $('#root');

function showLogin() {
  S.me = null;
  let need2fa = false, recovery = false;
  const user = h('input', { class: 'input', autocomplete: 'username', autocapitalize: 'none', spellcheck: 'false' });
  const pass = h('input', { class: 'input', type: 'password', autocomplete: 'current-password' });
  const code = h('input', { class: 'input code-input', inputmode: 'numeric', autocomplete: 'one-time-code', maxlength: '6', placeholder: '••••••' });
  const err = h('div', { class: 'err' });
  const btn = h('button', { class: 'btn primary' }, 'Sign in');
  const step1 = h('div', {}, h('label', { class: 'field' }, h('span', {}, 'Username'), user), h('label', { class: 'field' }, h('span', {}, 'Password'), pass));
  const toggle = h('a', { href: '#', onclick: (e) => { e.preventDefault(); recovery = !recovery; setCodeMode(); } });
  const step2 = h('div', { class: 'hidden' }, h('label', { class: 'field' }, code), h('div', { style: 'text-align:center;margin:-4px 0 14px;font-size:13px' }, toggle));
  const title = h('h1', {}, 'Welcome back');
  const sub = h('p', { class: 'sub' }, 'Sign in to your Vaultix drive');
  function setCodeMode() {
    code.value = '';
    if (recovery) { code.removeAttribute('maxlength'); code.placeholder = 'xxxxx-xxxxx'; code.inputMode = 'text'; toggle.textContent = 'Use authenticator code'; }
    else { code.setAttribute('maxlength', '6'); code.placeholder = '••••••'; code.inputMode = 'numeric'; toggle.textContent = 'Use a recovery code'; }
    code.focus();
  }
  async function submit(e) {
    e && e.preventDefault();
    err.textContent = '';
    btn.disabled = true;
    try {
      await api('POST', '/api/auth/login', { username: user.value, password: pass.value, code: need2fa ? code.value : undefined });
      await boot();
    } catch (ex) {
      if (ex.data && ex.data.totp_required) {
        if (!need2fa) {
          need2fa = true;
          step1.classList.add('hidden'); step2.classList.remove('hidden');
          title.textContent = 'Two-factor authentication';
          sub.textContent = 'Enter the 6-digit code from your authenticator app.';
          btn.textContent = 'Verify';
          setCodeMode();
          ex.message = '';
        }
      }
      err.textContent = ex.message;
      if (need2fa) { code.value = ''; code.focus(); }
    } finally { btn.disabled = false; }
  }
  code.addEventListener('input', () => { if (!recovery && /^\d{6}$/.test(code.value)) submit(); });
  root.replaceChildren(h('div', { class: 'login-wrap' }, VXTheme.button('btn ghost icon login-theme'), h('form', { class: 'login', onsubmit: submit },
    h('div', { class: 'brand' }, h('img', { src: '/icon.svg', alt: '' }), 'Vaultix'), title, sub, step1, step2, err, btn)), toastsEl);
  user.focus();
}

/* ================= shell ================= */

function closeSidebar() { const s = $('.side'); if (s) s.classList.remove('open'); }

function renderShell() {
  const nav = (r, ic, label) => h('button', { class: 'nav', 'data-r': r, onclick: () => { location.hash = r === 'files' ? '#/files/' : '#/' + r; } }, icon(ic), label);
  const usage = h('div', { class: 'usage', id: 'usage' });
  const side = h('aside', { class: 'side' },
    h('div', { class: 'brand' }, h('img', { src: '/icon.svg', alt: '' }), 'Vaultix'),
    nav('files', 'folder', 'All files'),
    nav('shared', 'link', 'Shared'),
    nav('trash', 'trash', 'Trash'),
    nav('settings', 'settings', 'Settings'),
    h('div', { class: 'grow' }),
    usage,
    h('div', { class: 'me' }, h('div', { class: 'avatar' }, S.me.username[0]), h('div', { class: 'name' }, S.me.username),
      VXTheme.button(), h('button', { class: 'btn ghost icon', title: 'Sign out', onclick: logout }, icon('logout'))));
  root.replaceChildren(h('div', { class: 'app' }, side, h('main', { class: 'main', id: 'main' })), toastsEl);
  loadUsage();
}

async function loadUsage() {
  try {
    const u = await api('GET', '/api/usage');
    const el = $('#usage'); if (!el) return;
    const total = u.total || 1;
    const pct = Math.min(100, ((total - u.free) / total) * 100);
    el.replaceChildren(h('div', {}, h('b', { style: 'color:var(--text);font-weight:600' }, fmtSize(u.used)), ' in your drive'),
      h('div', { class: 'bar' }, h('i', { style: `width:${pct.toFixed(1)}%` })), h('div', {}, fmtSize(u.free) + ' free of ' + fmtSize(u.total)));
  } catch {}
}

async function logout() {
  try { await api('POST', '/api/auth/logout'); } catch {}
  location.hash = '';
  showLogin();
}

/* ================= main area ================= */

let searchTimer;
function topBar(withCrumbs) {
  const crumbs = h('nav', { class: 'crumbs' });
  if (withCrumbs) {
    const parts = S.path ? S.path.split('/') : [];
    const mk = (label, p) => {
      const b = h('button', { onclick: () => go(p) }, label);
      dropTarget(b, p);
      return b;
    };
    crumbs.append(mk('All files', ''));
    parts.forEach((seg, i) => crumbs.append(h('span', { class: 'sep' }, '/'), mk(seg, parts.slice(0, i + 1).join('/'))));
  }
  const si = h('input', { class: 'input', id: 'search', placeholder: 'Search files', value: S.query, autocomplete: 'off', spellcheck: 'false' });
  si.addEventListener('input', () => {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      const q = si.value.trim();
      if (!q) { S.query = ''; if (S.route !== 'files') location.hash = '#/files/'; else loadFiles(); return; }
      if (S.route !== 'files') { S.route = 'files'; history.replaceState(null, '', '#/files/'); }
      doSearch(q);
    }, 220);
  });
  si.addEventListener('keydown', (e) => { if (e.key === 'Escape') { si.value = ''; si.dispatchEvent(new Event('input')); si.blur(); } });
  return h('header', { class: 'top' },
    h('button', { class: 'btn ghost icon mobile-only', onclick: () => $('.side').classList.toggle('open') }, icon('menu')),
    crumbs, h('div', { class: 'search' }, icon('search'), si, h('kbd', {}, '/')));
}

async function doSearch(q) {
  S.query = q; S.sel.clear();
  try { S.entries = (await api('GET', '/api/search?q=' + encodeURIComponent(q))).entries; } catch (e) { fail(e); }
  renderMain(true);
}

function renderMain(keepSearchFocus) {
  const main = $('#main'); if (!main) return;
  const active = document.activeElement && document.activeElement.id === 'search';
  if (S.route === 'trash') main.replaceChildren(topBar(false), ...trashView());
  else if (S.route === 'shared') main.replaceChildren(topBar(false), ...sharedView());
  else if (S.route === 'settings') main.replaceChildren(topBar(false), ...settingsView());
  else main.replaceChildren(topBar(!S.query), filesToolbar(), filesContent());
  if (active || keepSearchFocus) { const si = $('#search'); si.focus(); si.setSelectionRange(si.value.length, si.value.length); }
}

/* ================= files ================= */

function sorted() {
  const { key, dir } = S.sort;
  return [...S.entries].sort((a, b) => {
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    let r = 0;
    if (key === 'size') r = a.size - b.size;
    else if (key === 'mtime') r = a.mtime - b.mtime;
    if (!r) r = a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' });
    return r * dir;
  });
}

function filesToolbar() {
  const n = S.sel.size;
  if (n) {
    const items = selected();
    return h('div', { class: 'toolbar' }, h('div', { class: 'selbar' },
      h('button', { class: 'btn ghost icon', title: 'Clear selection', onclick: () => { S.sel.clear(); renderMain(); } }, icon('x')),
      h('span', { class: 'count' }, `${n} selected`),
      h('button', { class: 'btn', onclick: () => download(items) }, icon('download'), 'Download'),
      h('button', { class: 'btn', onclick: () => moveDialog(items) }, icon('move'), 'Move'),
      n === 1 && h('button', { class: 'btn', onclick: () => shareDialog(items[0].path, items[0].is_dir, 'link') }, icon('link'), 'Share'),
      n === 1 && h('button', { class: 'btn', onclick: () => rename(items[0]) }, icon('edit'), 'Rename'),
      h('button', { class: 'btn danger', onclick: () => remove(items) }, icon('trash'), 'Delete')));
  }
  const title = S.query ? `Results for “${S.query}”` : (S.path ? baseName(S.path) : 'All files');
  const seg = h('div', { class: 'seg' },
    h('button', { class: S.view === 'list' ? 'on' : '', title: 'List', onclick: () => setView('list') }, icon('list')),
    h('button', { class: S.view === 'grid' ? 'on' : '', title: 'Grid', onclick: () => setView('grid') }, icon('grid')));
  return h('div', { class: 'toolbar' }, h('div', { class: 'title' }, title),
    !S.query && S.path && h('button', { class: 'btn ghost icon', title: 'Share this folder', onclick: (ev) => { const r = ev.currentTarget.getBoundingClientRect(); openMenu(r.left, r.bottom + 6, [
      { icon: 'link', label: 'Share link…', fn: () => shareDialog(S.path, true, 'link') },
      { icon: 'upload', label: 'Request files…', fn: () => shareDialog(S.path, true, 'upload') },
    ]); } }, icon('link')),
    !S.query && h('button', { class: 'btn', onclick: newFolder }, icon('newfolder'), h('span', { class: 'hide-sm' }, 'New folder')),
    !S.query && h('button', { class: 'btn primary', onclick: (e) => { const r = e.currentTarget.getBoundingClientRect(); openMenu(r.left, r.bottom + 6, [
      { icon: 'upload', label: 'Upload files', fn: () => pickFiles(false) },
      { icon: 'folder', label: 'Upload folder', fn: () => pickFiles(true) },
    ]); } }, icon('upload'), 'Upload'),
    seg);
}
function setView(v) { S.view = v; try { localStorage.setItem('vx.view', v); } catch {} renderMain(); }
const selected = () => S.entries.filter((e) => S.sel.has(e.path));

function selectClick(e, ev, list) {
  if (ev.shiftKey && S.lastSel) {
    const a = list.findIndex((x) => x.path === S.lastSel), b = list.findIndex((x) => x.path === e.path);
    if (a >= 0 && b >= 0) for (let i = Math.min(a, b); i <= Math.max(a, b); i++) S.sel.add(list[i].path);
  } else if (S.sel.has(e.path)) S.sel.delete(e.path);
  else S.sel.add(e.path);
  S.lastSel = e.path;
  renderMain();
}

function itemMenu(ev, e) {
  ev.preventDefault();
  if (!S.sel.has(e.path)) { S.sel.clear(); S.sel.add(e.path); renderMain(); }
  const items = selected();
  const one = items.length === 1;
  openMenu(ev.clientX, ev.clientY, [
    one && { icon: e.is_dir ? 'folder' : 'open', label: e.is_dir ? 'Open' : 'Preview', fn: () => open(e) },
    { icon: 'download', label: 'Download', fn: () => download(items) },
    one && { icon: 'link', label: 'Share link…', fn: () => shareDialog(e.path, e.is_dir, 'link') },
    one && e.is_dir && { icon: 'upload', label: 'Request files…', fn: () => shareDialog(e.path, true, 'upload') },
    one && '-',
    one && { icon: 'edit', label: 'Rename', fn: () => rename(e) },
    { icon: 'move', label: 'Move to…', fn: () => moveDialog(items) },
    S.query && one && { icon: 'folder', label: 'Show in folder', fn: () => { S.query = ''; go(parentOf(e.path)); } },
    '-',
    { icon: 'trash', label: 'Delete', danger: true, fn: () => remove(items) },
  ]);
}

function draggable(el, e) {
  el.draggable = true;
  el.addEventListener('dragstart', (ev) => {
    if (!S.sel.has(e.path)) { S.sel.clear(); S.sel.add(e.path); }
    ev.dataTransfer.setData('application/x-vaultix', JSON.stringify([...S.sel]));
    ev.dataTransfer.effectAllowed = 'move';
  });
}
function dropTarget(el, dest) {
  el.addEventListener('dragover', (ev) => {
    const internal = ev.dataTransfer.types.includes('application/x-vaultix');
    if (!internal && !ev.dataTransfer.types.includes('Files')) return;
    ev.preventDefault(); ev.stopPropagation();
    ev.dataTransfer.dropEffect = internal ? 'move' : 'copy';
    el.classList.add('drop');
    hideDropzone();
  });
  el.addEventListener('dragleave', () => el.classList.remove('drop'));
  el.addEventListener('drop', async (ev) => {
    el.classList.remove('drop');
    const data = ev.dataTransfer.getData('application/x-vaultix');
    ev.preventDefault(); ev.stopPropagation();
    hideDropzone();
    if (data) {
      const paths = JSON.parse(data).filter((p) => p !== dest && parentOf(p) !== dest);
      if (paths.length) await moveTo(paths, dest);
    } else {
      queueUploads(await collectDropped(ev.dataTransfer), dest);
    }
  });
}

function filesContent() {
  const list = sorted();
  const content = h('div', { class: 'content' + (S.sel.size ? ' any-sel' : '') });
  if (!list.length) {
    content.append(S.query
      ? h('div', { class: 'empty' }, h('div', { class: 'big' }, icon('search')), h('h3', {}, 'No matches'), h('div', {}, 'Try a different name.'))
      : h('div', { class: 'empty' }, h('div', { class: 'big' }, icon('upload')), h('h3', {}, 'This folder is empty'), h('div', {}, 'Drop files here, or use Upload.')));
    return content;
  }
  const ck = (e) => h('div', { class: 'ck', onclick: (ev) => { ev.stopPropagation(); selectClick(e, ev, list); } }, icon('check'));
  const onClick = (e) => (ev) => {
    if (ev.metaKey || ev.ctrlKey || ev.shiftKey || S.sel.size) { selectClick(e, ev, list); return; }
    open(e);
  };
  if (S.view === 'grid') {
    const g = h('div', { class: 'grid' });
    for (const e of list) {
      const k = kind(e);
      const thumb = k === 'image' && e.size < 40e6 && ext(e.name) !== 'heic'
        ? h('img', { src: fileUrl(e.path), loading: 'lazy', decoding: 'async', alt: '' })
        : ficon(e, true);
      const t = h('div', { class: 'tile' + (S.sel.has(e.path) ? ' sel' : ''), onclick: onClick(e), oncontextmenu: (ev) => itemMenu(ev, e) },
        ck(e), h('div', { class: 'thumb', style: '--tint:' + tint(e) }, thumb),
        h('div', { class: 'cap' }, h('b', { title: e.name }, S.sharedPaths.has(e.path) && h('span', { class: 'shared-badge', title: 'Shared' }, icon('link')), e.name), h('small', {}, e.is_dir ? (S.query ? parentOf(e.path) || 'All files' : 'Folder') : fmtSize(e.size) + ' · ' + fmtDate(e.mtime))));
      draggable(t, e);
      if (e.is_dir) dropTarget(t, e.path);
      g.append(t);
    }
    content.append(g);
  } else {
    const th = (key, label, cls = '') => h('th', { class: cls, onclick: () => { S.sort = { key, dir: S.sort.key === key ? -S.sort.dir : 1 }; renderMain(); } },
      label, S.sort.key === key ? (S.sort.dir > 0 ? ' ↑' : ' ↓') : '');
    const allSel = list.length && list.every((e) => S.sel.has(e.path));
    const tbody = h('tbody');
    const table = h('table', { class: 'list' }, h('thead', {}, h('tr', {},
      h('th', { style: 'width:1%', class: allSel ? 'sel' : '', onclick: () => { if (allSel) S.sel.clear(); else list.forEach((e) => S.sel.add(e.path)); renderMain(); } }, h('div', { class: 'ck', style: S.sel.size ? 'opacity:1' : '' }, icon('check'))),
      th('name', 'Name'), th('size', 'Size', 'hide-sm'), th('mtime', 'Modified'), h('th'))), tbody);
    for (const e of list) {
      const tr = h('tr', { class: 'row' + (S.sel.has(e.path) ? ' sel' : ''), onclick: onClick(e), oncontextmenu: (ev) => itemMenu(ev, e) },
        h('td', {}, ck(e)),
        h('td', { class: 'name' }, h('div', { class: 'nm' }, ficon(e), h('div', { style: 'min-width:0;display:flex;flex-direction:column' },
          h('span', { title: e.name }, e.name, S.sharedPaths.has(e.path) && h('span', { class: 'shared-badge', title: 'Shared' }, icon('link'))), S.query && h('small', { style: 'color:var(--faint);font-size:12px;overflow:hidden;text-overflow:ellipsis' }, parentOf(e.path) || 'All files')))),
        h('td', { class: 'meta hide-sm' }, e.is_dir ? '—' : fmtSize(e.size)),
        h('td', { class: 'meta' }, fmtDate(e.mtime)),
        h('td', { class: 'act' }, h('button', { class: 'btn ghost icon more', onclick: (ev) => { ev.stopPropagation(); itemMenu(ev, e); } }, icon('more'))));
      draggable(tr, e);
      if (e.is_dir) dropTarget(tr, e.path);
      tbody.append(tr);
    }
    content.append(table);
  }
  return content;
}

function open(e) {
  if (e.is_dir) { S.query = ''; go(e.path); return; }
  preview(e);
}

function download(items) {
  if (items.length === 1 && !items[0].is_dir) location.href = fileUrl(items[0].path, true);
  else location.href = '/api/zip?paths=' + encodeURIComponent(JSON.stringify(items.map((i) => i.path)));
}

async function newFolder() {
  const name = await prompt2('New folder', { label: 'Name', value: 'Untitled folder', ok: 'Create' });
  if (!name) return;
  try { await api('POST', '/api/mkdir', { path: joinPath(S.path, name) }); await loadFiles(); } catch (e) { fail(e); }
}

async function rename(e) {
  const name = await prompt2('Rename', { label: 'New name', value: e.name, selectStem: !e.is_dir });
  if (!name || name === e.name) return;
  if (name.includes('/')) return toast('Names can’t contain “/”', { error: true });
  try { await api('POST', '/api/move', { from: e.path, to: joinPath(parentOf(e.path), name) }); S.sel.clear(); await refresh(); } catch (er) { fail(er); }
}

async function remove(items) {
  try {
    const r = await api('POST', '/api/delete', { paths: items.map((i) => i.path) });
    S.sel.clear();
    await refresh();
    loadUsage();
    toast(`Moved ${items.length === 1 ? '“' + items[0].name + '”' : items.length + ' items'} to Trash`, {
      ms: 6000,
      action: r.trash_ids && r.trash_ids.length ? { label: 'Undo', fn: async () => { try { await api('POST', '/api/trash/restore', { ids: r.trash_ids }); await refresh(); } catch (e) { fail(e); } } } : null,
    });
  } catch (e) { fail(e); }
}

async function moveTo(paths, dest) {
  let moved = 0;
  for (const p of paths) {
    try { await api('POST', '/api/move', { from: p, to: joinPath(dest, baseName(p)) }); moved++; } catch (e) { fail(new Error(`${baseName(p)}: ${e.message}`)); }
  }
  S.sel.clear();
  await refresh();
  if (moved) toast(`Moved ${moved} item${moved > 1 ? 's' : ''} to ${dest ? baseName(dest) : 'All files'}`);
}

function moveDialog(items) {
  const moving = new Set(items.map((i) => i.path));
  modal((m, close) => {
    let cur = parentOf(items[0].path);
    const box = h('div', { class: 'folder-pick' });
    const okBtn = h('button', { class: 'btn primary', onclick: async () => { close(); await moveTo([...moving], cur); } }, 'Move here');
    async function load() {
      box.replaceChildren(h('div', { class: 'none' }, 'Loading…'));
      let dirs = [];
      try { dirs = (await api('GET', '/api/list?path=' + encodeURIComponent(cur))).entries.filter((e) => e.is_dir && !moving.has(e.path)); } catch (e) { fail(e); }
      dirs.sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }));
      box.replaceChildren(
        h('div', { class: 'head' }, cur && h('button', { class: 'btn ghost icon', style: 'height:26px;width:26px', onclick: () => { cur = parentOf(cur); load(); } }, icon('left')), cur ? baseName(cur) : 'All files'),
        dirs.length ? dirs.map((d) => h('button', { class: 'f', onclick: () => { cur = d.path; load(); } }, ficon(d), d.name)) : h('div', { class: 'none' }, 'No subfolders'));
      okBtn.disabled = items.every((i) => parentOf(i.path) === cur);
    }
    m.append(h('h2', {}, `Move ${items.length > 1 ? items.length + ' items' : '“' + items[0].name + '”'}`), h('p', {}, 'Choose a destination folder.'), box,
      h('div', { class: 'actions' }, h('button', { class: 'btn', onclick: () => close() }, 'Cancel'), okBtn));
    load();
  });
}

/* ================= preview ================= */

function preview(e) {
  const files = sorted().filter((x) => !x.is_dir);
  VXViewer.open({
    files, index: Math.max(0, files.findIndex((x) => x.path === e.path)),
    url: (f) => fileUrl(f.path),
    meta: (f) => fmtSize(f.size) + ' · ' + fmtDate(f.mtime),
    download: (f) => download([f]),
  });
}

/* ================= uploads ================= */

const UP = { queue: [], active: 0, items: [], panel: null, total: 0, done: 0, dirs: new Set() };

function pickFiles(folder) {
  const inp = h('input', { type: 'file', multiple: true, webkitdirectory: folder || null, style: 'display:none' });
  inp.addEventListener('change', () => {
    const list = [...inp.files].map((f) => ({ file: f, rel: folder ? f.webkitRelativePath : f.name }));
    queueUploads(list, S.path);
    inp.remove();
  });
  document.body.append(inp);
  inp.click();
}

async function collectDropped(dt) {
  const entries = [...dt.items].map((i) => i.webkitGetAsEntry && i.webkitGetAsEntry()).filter(Boolean);
  if (!entries.length) return [...dt.files].map((f) => ({ file: f, rel: f.name }));
  const out = [];
  const readAll = (reader) => new Promise((res) => { const acc = []; const next = () => reader.readEntries((b) => { if (!b.length) res(acc); else { acc.push(...b); next(); } }, () => res(acc)); next(); });
  async function walk(entry, prefix) {
    if (entry.isFile) {
      await new Promise((res) => entry.file((f) => { out.push({ file: f, rel: prefix + f.name }); res(); }, res));
    } else if (entry.isDirectory) {
      const kids = await readAll(entry.createReader());
      if (!kids.length) out.push({ dir: true, rel: prefix + entry.name });
      for (const k of kids) await walk(k, prefix + entry.name + '/');
    }
  }
  for (const e of entries) await walk(e, '');
  return out;
}

function queueUploads(list, dest) {
  list = list.filter((x) => x.dir || !/^(\.DS_Store|Thumbs\.db|desktop\.ini)$/.test(baseName(x.rel)));
  if (!list.length) return;
  if (UP.active === 0 && !UP.queue.length) { UP.items = []; UP.total = 0; UP.done = 0; }
  for (const it of list) {
    const path = joinPath(dest, it.rel);
    if (it.dir) { api('POST', '/api/mkdir', { path }).catch(fail); continue; }
    const item = { file: it.file, path, loaded: 0, state: 'queued', el: null };
    UP.items.push(item); UP.queue.push(item); UP.total++;
  }
  renderUploads();
  pump();
}

function pump() {
  while (UP.active < 3 && UP.queue.length) {
    const it = UP.queue.shift();
    UP.active++;
    it.state = 'up';
    const xhr = new XMLHttpRequest();
    xhr.open('PUT', '/api/file?path=' + encodeURIComponent(it.path) + '&mtime=' + (it.file.lastModified || Date.now()));
    xhr.setRequestHeader('x-vaultix', '1');
    xhr.upload.onprogress = (e) => { it.loaded = e.loaded; updateUploadItem(it); };
    xhr.onloadend = () => {
      UP.active--;
      UP.done++;
      if (xhr.status >= 200 && xhr.status < 300) { it.state = 'done'; it.loaded = it.file.size; }
      else { it.state = 'fail'; let m = 'Upload failed'; try { m = JSON.parse(xhr.responseText).error; } catch {} it.err = m; if (xhr.status === 401) showLogin(); }
      updateUploadItem(it); updateUploadHead();
      if (!UP.active && !UP.queue.length) finishUploads();
      else pump();
    };
    xhr.send(it.file);
  }
  updateUploadHead();
}

function renderUploads() {
  if (!UP.panel) {
    UP.head = h('span');
    UP.list = h('div', { class: 'ul' });
    UP.panel = h('div', { class: 'uploads' }, h('div', { class: 'uh' }, icon('upload'), UP.head,
      h('button', { class: 'btn ghost icon', style: 'height:28px;width:28px', onclick: () => { UP.panel.remove(); UP.panel = null; } }, icon('x'))), UP.list);
    toastsEl.append(UP.panel);
  }
  for (const it of UP.items) if (!it.el) { it.el = h('div', { class: 'ui' }); UP.list.append(it.el); updateUploadItem(it); }
  updateUploadHead();
}
function updateUploadItem(it) {
  if (!it.el) return;
  const pct = it.file.size ? (it.loaded / it.file.size) * 100 : it.state === 'done' ? 100 : 0;
  it.el.className = 'ui ' + it.state;
  it.el.replaceChildren(h('div', { class: 'r' }, h('b', { title: it.path }, baseName(it.path)),
    h('small', {}, it.state === 'fail' ? it.err : it.state === 'done' ? fmtSize(it.file.size) : it.state === 'queued' ? 'Waiting' : Math.floor(pct) + '%')),
    h('div', { class: 'bar' }, h('i', { style: `width:${it.state === 'fail' ? 100 : pct}%` })));
}
function updateUploadHead() {
  if (!UP.head) return;
  const failed = UP.items.filter((i) => i.state === 'fail').length;
  UP.head.textContent = UP.done < UP.total ? `Uploading ${UP.done + 1 > UP.total ? UP.total : UP.done + 1} of ${UP.total}` : failed ? `${failed} upload${failed > 1 ? 's' : ''} failed` : `Uploaded ${UP.total} item${UP.total > 1 ? 's' : ''}`;
}
let refreshTimer;
function finishUploads() {
  clearTimeout(refreshTimer);
  if (S.route === 'files' && !S.query) loadFiles();
  loadUsage();
  if (!UP.items.some((i) => i.state === 'fail')) setTimeout(() => { if (UP.panel && !UP.active && !UP.queue.length) { UP.panel.remove(); UP.panel = null; } }, 3500);
}

// Page-wide drop zone for uploads into the current folder.
let dz = null, dzDepth = 0;
function hideDropzone() { if (dz) { dz.remove(); dz = null; } dzDepth = 0; }
addEventListener('dragenter', (e) => {
  if (!S.me || S.route !== 'files' || S.query || !e.dataTransfer.types.includes('Files')) return;
  dzDepth++;
  if (!dz) { dz = h('div', { class: 'dropzone' }, h('div', {}, icon('upload'), 'Drop to upload to ', h('span', { style: 'color:var(--accent)' }, S.path ? baseName(S.path) : 'All files'))); document.body.append(dz); }
});
addEventListener('dragleave', () => { if (dz && --dzDepth <= 0) hideDropzone(); });
addEventListener('dragover', (e) => { if (e.dataTransfer.types.includes('Files')) e.preventDefault(); });
addEventListener('drop', async (e) => {
  if (!e.dataTransfer.types.includes('Files')) return;
  e.preventDefault();
  hideDropzone();
  if (!S.me || S.route !== 'files' || S.query) return;
  queueUploads(await collectDropped(e.dataTransfer), S.path);
});

/* ================= trash ================= */

function trashView() {
  const content = h('div', { class: 'content' }, h('div', { class: 'empty' }, 'Loading…'));
  const toolbar = h('div', { class: 'toolbar' }, h('div', { class: 'title' }, 'Trash'));
  (async () => {
    let items = [];
    try { items = (await api('GET', '/api/trash')).items; } catch (e) { fail(e); }
    const reload = () => renderMain();
    if (items.length) toolbar.append(h('button', { class: 'btn danger', onclick: async () => {
      if (!(await confirm2('Empty trash?', `${items.length} item${items.length > 1 ? 's' : ''} will be permanently deleted. This can’t be undone.`, { ok: 'Empty trash', danger: true }))) return;
      try { await api('POST', '/api/trash/purge', { all: true }); loadUsage(); reload(); } catch (e) { fail(e); }
    } }, icon('trash'), 'Empty trash'));
    if (!items.length) {
      content.replaceChildren(h('div', { class: 'empty' }, h('div', { class: 'big' }, icon('trash')), h('h3', {}, 'Trash is empty'), h('div', {}, 'Deleted items stay here for 30 days.')));
      return;
    }
    const tbody = h('tbody');
    for (const it of items) {
      const e = { name: baseName(it.path), is_dir: it.is_dir };
      tbody.append(h('tr', { class: 'row' },
        h('td', { class: 'name' }, h('div', { class: 'nm' }, ficon(e), h('div', { style: 'min-width:0;display:flex;flex-direction:column' }, h('span', {}, e.name),
          h('small', { style: 'color:var(--faint);font-size:12px' }, parentOf(it.path) || 'All files')))),
        h('td', { class: 'meta hide-sm' }, fmtSize(it.size)),
        h('td', { class: 'meta' }, fmtDate(it.deleted_at * 1000)),
        h('td', { class: 'act', style: 'width:auto' }, h('div', { style: 'display:flex;gap:6px;justify-content:flex-end' },
          h('button', { class: 'btn', onclick: async () => { try { await api('POST', '/api/trash/restore', { ids: [it.id] }); toast('Restored ' + e.name); reload(); } catch (er) { fail(er); } } }, icon('restore'), 'Restore'),
          h('button', { class: 'btn ghost icon danger', title: 'Delete forever', onclick: async () => {
            if (!(await confirm2('Delete forever?', `“${e.name}” will be permanently deleted.`, { ok: 'Delete', danger: true }))) return;
            try { await api('POST', '/api/trash/purge', { ids: [it.id] }); loadUsage(); reload(); } catch (er) { fail(er); }
          } }, icon('x'))))));
    }
    content.replaceChildren(h('table', { class: 'list' }, h('thead', {}, h('tr', {}, h('th', {}, 'Name'), h('th', { class: 'hide-sm' }, 'Size'), h('th', {}, 'Deleted'), h('th'))), tbody));
  })();
  return [toolbar, content];
}

/* ================= sharing ================= */

async function loadShares() {
  try {
    S.shares = (await api('GET', '/api/shares')).shares;
    S.sharedPaths = new Set(S.shares.filter((x) => !x.expired).map((x) => x.path));
  } catch {}
}
const shareUrl = (sh) => location.origin + sh.url;
const DAY = 86400;
const SIZE_LIMITS = [['', 'No limit'], [100 * 2 ** 20, '100 MB'], [2 ** 30, '1 GB'], [5 * 2 ** 30, '5 GB'], [20 * 2 ** 30, '20 GB']];

async function copyText(t, msg = 'Link copied') {
  try { await navigator.clipboard.writeText(t); toast(msg); } catch { prompt2('Copy this link', { value: t, ok: 'Done' }); }
}

async function shareDialog(path, isDir, kind) {
  await loadShares();
  let sh = S.shares.find((x) => x.path === path && x.kind === kind && !x.expired) || null;
  const name = baseName(path) || 'All files';
  await modal((m, close) => {
    const pw = h('input', { class: 'input', type: 'password', autocomplete: 'new-password' });
    const rmPw = h('input', { type: 'checkbox' });
    const exp = h('select', { class: 'input' },
      [['never', 'Never'], ['1', 'In 1 day'], ['7', 'In 7 days'], ['30', 'In 30 days'], ['custom', 'On a date…']].map(([v, l]) => h('option', { value: v }, l)));
    const expDate = h('input', { class: 'input', type: 'date' });
    const dl = h('input', { type: 'checkbox' });
    const note = h('textarea', { class: 'input', rows: '2', placeholder: 'e.g. Please send the STL files for the bracket.' });
    const max = h('select', { class: 'input' }, SIZE_LIMITS.map(([v, l]) => h('option', { value: String(v) }, l)));
    const syncExp = () => expDate.classList.toggle('hidden', exp.value !== 'custom');
    exp.addEventListener('change', syncExp);
    function fill() {
      pw.value = '';
      pw.placeholder = sh && sh.has_password ? 'Password set (type to change)' : 'No password';
      rmPw.checked = false;
      dl.checked = sh ? sh.allow_download : true;
      note.value = (sh && sh.note) || '';
      max.value = sh && sh.max_size ? String(sh.max_size) : '';
      if (sh && sh.expires_at) {
        exp.value = 'custom';
        expDate.value = new Date(sh.expires_at * 1000).toISOString().slice(0, 10);
      } else { exp.value = 'never'; expDate.value = ''; }
      syncExp();
    }
    function payload() {
      let expires_at = null;
      if (exp.value === 'custom' && expDate.value) expires_at = Math.floor(new Date(expDate.value + 'T23:59:59').getTime() / 1000);
      else if (exp.value !== 'never' && exp.value !== 'custom') expires_at = Math.floor(Date.now() / 1000) + Number(exp.value) * DAY;
      const p = { path, kind, expires_at, allow_download: dl.checked, note: note.value, max_size: max.value ? Number(max.value) : null };
      if (pw.value) p.password = pw.value; else if (rmPw.checked) p.password = '';
      return p;
    }
    const err = h('div', { class: 'err' });
    async function save(create) {
      err.textContent = '';
      try {
        const p = payload();
        if (create) { sh = await api('POST', '/api/shares', p); copyText(shareUrl(sh)); }
        else { sh = await api('POST', '/api/shares/update', { ...p, id: sh.id }); toast('Link updated'); }
        await loadShares();
        render();
      } catch (e) { err.textContent = e.message; }
    }
    function render() {
      const title = kind === 'link' ? `Share “${name}”` : `Request files into “${name}”`;
      const desc = kind === 'link'
        ? `Anyone with the link can view this ${isDir ? 'folder' : 'file'}.`
        : 'Anyone with the link can upload files into this folder. They can’t see what’s already in it.';
      const linkBox = sh && h('div', { class: 'linkbox' },
        h('input', { class: 'input mono-in', readonly: true, value: shareUrl(sh), onfocus: (e) => e.target.select() }),
        h('button', { class: 'btn primary', onclick: () => copyText(shareUrl(sh)) }, 'Copy'),
        h('a', { class: 'btn icon', href: shareUrl(sh), target: '_blank', rel: 'noopener', title: 'Open link' }, icon('open')));
      const stats = sh && h('div', { class: 'share-stats' }, kind === 'link'
        ? `${sh.views} view${sh.views === 1 ? '' : 's'}`
        : `${sh.uploads} file${sh.uploads === 1 ? '' : 's'} received`, ` · created ${fmtDate(sh.created_at * 1000)}`);
      m.replaceChildren(
        h('h2', {}, title), h('p', {}, desc), linkBox || '', stats || '',
        h('div', { class: 'opts' },
          h('div', { class: 'field' }, h('span', {}, 'Password'), pw,
            sh && sh.has_password && h('label', { class: 'chk' }, rmPw, 'Remove password')),
          h('div', { class: 'row2' },
            h('label', { class: 'field' }, h('span', {}, 'Link expires'), exp, expDate),
            kind === 'link'
              ? h('div', { class: 'field' }, h('span', {}, 'Permissions'), h('label', { class: 'chk', style: 'height:40px;margin:0' }, dl, 'Allow downloads'))
              : h('label', { class: 'field' }, h('span', {}, 'Max file size'), max)),
          kind === 'upload' && h('label', { class: 'field' }, h('span', {}, 'Message for uploaders'), note)),
        err,
        h('div', { class: 'actions' },
          sh && h('button', { class: 'btn danger', style: 'margin-right:auto', onclick: async () => {
            if (!(await confirm2('Delete this link?', 'People with the link will no longer be able to open it.', { ok: 'Delete link', danger: true }))) return;
            try { await api('POST', '/api/shares/revoke', { id: sh.id }); await loadShares(); toast('Link deleted'); close(); renderMain(); } catch (e) { fail(e); }
          } }, 'Delete link'),
          h('button', { class: 'btn', onclick: () => { close(); renderMain(); } }, sh ? 'Done' : 'Cancel'),
          h('button', { class: 'btn primary', onclick: () => save(!sh) }, sh ? 'Save changes' : (kind === 'link' ? 'Create link' : 'Create request link'))));
      fill();
    }
    m.classList.add('wide');
    render();
  });
}

function sharedView() {
  const content = h('div', { class: 'content' }, h('div', { class: 'empty' }, 'Loading…'));
  const toolbar = h('div', { class: 'toolbar' }, h('div', { class: 'title' }, 'Shared'));
  (async () => {
    await loadShares();
    if (!S.shares.length) {
      content.replaceChildren(h('div', { class: 'empty' }, h('div', { class: 'big' }, icon('link')), h('h3', {}, 'Nothing shared yet'),
        h('div', {}, 'Right-click a file or folder and choose “Share link…” or “Request files…”.')));
      return;
    }
    const tbody = h('tbody');
    for (const sh of S.shares) {
      const e = { name: sh.name, is_dir: sh.is_dir };
      const sub = [];
      if (!sh.exists) sub.push(h('span', { class: 'warn' }, 'Item no longer exists'));
      else sub.push(parentOf(sh.path) || 'All files');
      if (sh.has_password) sub.push(' · 🔒 password');
      if (sh.kind === 'upload' && sh.recent_uploads.length) {
        const u = sh.recent_uploads[0];
        sub.push(` · last: ${u.name}${u.uploader ? ' from ' + u.uploader : ''}`);
      }
      const expires = sh.expired ? h('span', { class: 'warn' }, 'Expired') : sh.expires_at ? fmtDate(sh.expires_at * 1000) : 'Never';
      tbody.append(h('tr', { class: 'row' + (sh.expired || !sh.exists ? ' dim' : '') },
        h('td', { class: 'name' }, h('div', { class: 'nm' }, ficon(e), h('div', { style: 'min-width:0;display:flex;flex-direction:column' },
          h('span', {}, sh.name), h('small', { style: 'color:var(--faint);font-size:12px;overflow:hidden;text-overflow:ellipsis' }, sub)))),
        h('td', {}, h('span', { class: 'pill ' + (sh.kind === 'link' ? 'p-link' : 'p-up') }, sh.kind === 'link' ? 'Link' : 'File request')),
        h('td', { class: 'meta hide-sm' }, sh.kind === 'link' ? `${sh.views} view${sh.views === 1 ? '' : 's'}` : `${sh.uploads} received`),
        h('td', { class: 'meta hide-sm' }, expires),
        h('td', { class: 'act', style: 'width:auto' }, h('div', { style: 'display:flex;gap:6px;justify-content:flex-end' },
          h('button', { class: 'btn', onclick: () => copyText(shareUrl(sh)) }, 'Copy link'),
          h('button', { class: 'btn ghost icon', title: 'Edit', onclick: async () => { await shareDialog(sh.path, sh.is_dir, sh.kind); renderMain(); } }, icon('settings')),
          h('button', { class: 'btn ghost icon danger', title: 'Delete link', onclick: async () => {
            if (!(await confirm2('Delete this link?', `“${sh.name}” will no longer be reachable through it.`, { ok: 'Delete link', danger: true }))) return;
            try { await api('POST', '/api/shares/revoke', { id: sh.id }); renderMain(); } catch (er) { fail(er); }
          } }, icon('x'))))));
    }
    content.replaceChildren(h('table', { class: 'list' }, h('thead', {}, h('tr', {}, h('th', {}, 'Name'), h('th', {}, 'Type'), h('th', { class: 'hide-sm' }, 'Activity'), h('th', { class: 'hide-sm' }, 'Expires'), h('th'))), tbody));
  })();
  return [toolbar, content];
}

/* ================= settings ================= */

function settingsView() {
  const wrap = h('div', { class: 'settings' });
  const content = h('div', { class: 'content' }, wrap);
  const toolbar = h('div', { class: 'toolbar' }, h('div', { class: 'title' }, 'Settings'));
  const cards = { account: h('div', { class: 'card' }), appearance: h('div', { class: 'card' }), tfa: h('div', { class: 'card' }), connect: h('div', { class: 'card' }), devices: h('div', { class: 'card' }), sessions: h('div', { class: 'card' }) };
  wrap.append(cards.account, cards.appearance, cards.tfa, cards.connect, cards.devices, cards.sessions);

  // Appearance
  {
    const opts = [['auto', 'System'], ['light', 'Light'], ['dark', 'Dark']];
    const seg = h('div', { class: 'seg seg-text' });
    const paint = () => seg.querySelectorAll('button').forEach((b) => b.classList.toggle('on', b.dataset.v === VXTheme.pref()));
    opts.forEach(([v, label]) => seg.append(h('button', { type: 'button', 'data-v': v, onclick: () => VXTheme.set(v) }, label)));
    paint();
    window.addEventListener('vx-theme', paint);
    cards.appearance.append(h('h3', {}, icon('sun'), 'Appearance'),
      h('p', {}, 'System follows your device’s light/dark setting. The sun/moon button in the sidebar switches quickly. Saved in this browser.'), seg);
  }

  // Account / password
  {
    const cur = h('input', { class: 'input', type: 'password', autocomplete: 'current-password' });
    const nw = h('input', { class: 'input', type: 'password', autocomplete: 'new-password' });
    const btn = h('button', { class: 'btn primary' }, 'Update password');
    cards.account.append(h('h3', {}, icon('user'), 'Account'), h('p', {}, `Signed in as ${S.me.username}${S.me.is_admin ? ' (admin)' : ''}.`),
      h('form', { onsubmit: async (e) => {
        e.preventDefault(); btn.disabled = true;
        try { await api('POST', '/api/auth/password', { current: cur.value, new: nw.value }); cur.value = nw.value = ''; toast('Password updated. Other browsers were signed out.'); } catch (er) { fail(er); }
        btn.disabled = false;
      } }, h('div', { class: 'row2' }, h('label', { class: 'field' }, h('span', {}, 'Current password'), cur), h('label', { class: 'field' }, h('span', {}, 'New password'), nw)), btn));
  }

  // 2FA
  function renderTfa() {
    const c = cards.tfa;
    c.replaceChildren(h('h3', {}, icon('shield'), 'Two-factor authentication', h('span', { class: 'pill' + (S.me.totp_enabled ? ' on' : '') }, S.me.totp_enabled ? 'On' : 'Off')));
    if (S.me.totp_enabled) {
      c.append(h('p', {}, `Your account requires a code from your authenticator app. ${S.me.recovery_codes_left} recovery code${S.me.recovery_codes_left === 1 ? '' : 's'} left.`),
        h('button', { class: 'btn danger', onclick: async () => {
          const pw = await modal((m, close) => {
            const inp = h('input', { class: 'input', type: 'password' });
            m.append(h('form', { onsubmit: (e) => { e.preventDefault(); close(inp.value); } }, h('h2', {}, 'Turn off 2FA'), h('p', {}, 'Confirm your password to disable two-factor authentication.'),
              h('label', { class: 'field' }, inp), h('div', { class: 'actions' }, h('button', { type: 'button', class: 'btn', onclick: () => close(null) }, 'Cancel'), h('button', { class: 'btn danger solid' }, 'Turn off'))));
          });
          if (!pw) return;
          try { await api('POST', '/api/2fa/disable', { password: pw }); S.me.totp_enabled = false; renderTfa(); toast('Two-factor authentication turned off'); } catch (e) { fail(e); }
        } }, 'Turn off 2FA'));
    } else {
      c.append(h('p', {}, 'Protect your drive with a 6-digit code from an authenticator app (1Password, Authy, Google Authenticator…).'),
        h('button', { class: 'btn primary', onclick: setupTfa }, icon('shield'), 'Set up 2FA'));
    }
  }
  async function setupTfa() {
    let d;
    try { d = await api('POST', '/api/2fa/setup'); } catch (e) { return fail(e); }
    const c = cards.tfa;
    const code = h('input', { class: 'input code-input', inputmode: 'numeric', maxlength: '6', placeholder: '••••••', autocomplete: 'one-time-code' });
    const err = h('div', { class: 'err' });
    const verify = async (e) => {
      e && e.preventDefault();
      try {
        const r = await api('POST', '/api/2fa/enable', { code: code.value });
        S.me.totp_enabled = true; S.me.recovery_codes_left = r.recovery_codes.length;
        c.replaceChildren(h('h3', {}, icon('shield'), 'Save your recovery codes', h('span', { class: 'pill on' }, 'On')),
          h('p', {}, 'Each code works once if you lose your phone. Store them somewhere safe; they won’t be shown again.'),
          h('div', { class: 'codes' }, r.recovery_codes.map((x) => h('div', {}, x))),
          h('div', { style: 'display:flex;gap:8px' },
            h('button', { class: 'btn', onclick: () => { navigator.clipboard.writeText(r.recovery_codes.join('\n')); toast('Copied'); } }, 'Copy codes'),
            h('button', { class: 'btn primary', onclick: renderTfa }, 'Done')));
      } catch (ex) { err.textContent = ex.message; code.value = ''; code.focus(); }
    };
    code.addEventListener('input', () => { if (/^\d{6}$/.test(code.value)) verify(); });
    c.replaceChildren(h('h3', {}, icon('shield'), 'Set up two-factor authentication'),
      h('p', {}, 'Scan this QR code with your authenticator app, then enter the 6-digit code it shows.'),
      h('div', { class: 'qr' }, h('img', { src: d.qr, alt: 'QR code' }), h('div', { style: 'flex:1;min-width:0' }, h('div', { style: 'font-size:12.5px;color:var(--muted);margin-bottom:6px' }, 'Or enter this key manually:'), h('div', { class: 'mono' }, d.secret))),
      h('form', { onsubmit: verify, style: 'max-width:260px' }, code, err, h('div', { style: 'display:flex;gap:8px' }, h('button', { type: 'button', class: 'btn', onclick: renderTfa }, 'Cancel'), h('button', { class: 'btn primary' }, 'Verify & enable'))));
    code.focus();
  }
  renderTfa();

  // Connect (WebDAV + desktop)
  {
    const o = location.origin;
    cards.connect.append(h('h3', {}, icon('link'), 'Connect apps'),
      h('p', {}, 'Use an app password (below) with these addresses. Your account password also works for WebDAV when 2FA is off.'),
      h('div', { class: 'kv' },
        h('span', {}, 'WebDAV'), h('div', { class: 'mono' }, o + '/dav/'),
        h('span', {}, 'Nextcloud-style'), h('div', { class: 'mono' }, `${o}/remote.php/dav/files/${S.me.username}/`),
        h('span', {}, 'Desktop sync'), h('div', { class: 'mono' }, o)));
  }

  // App passwords / devices
  async function loadDevices() {
    let d;
    try { d = await api('GET', '/api/devices'); } catch (e) { return fail(e); }
    cards.devices.replaceChildren(h('h3', {}, icon('key'), 'Devices & app passwords'),
      h('p', {}, 'Desktop sync clients and WebDAV apps each get their own password, so you can revoke one without affecting the others.'),
      h('div', { class: 'items' }, d.tokens.length ? d.tokens.map((t) => h('div', { class: 'it' }, h('div', { class: 'ficon chip' }, icon('monitor')),
        h('div', { class: 't' }, h('b', {}, t.name), h('small', {}, `Created ${fmtDate(t.created_at * 1000)} · ${t.last_used ? 'last used ' + fmtDate(t.last_used * 1000) : 'never used'}`)),
        h('button', { class: 'btn danger', onclick: async () => {
          if (!(await confirm2('Revoke access?', `“${t.name}” will be signed out immediately.`, { ok: 'Revoke', danger: true }))) return;
          try { await api('POST', '/api/devices/revoke', { token_id: t.id }); loadDevices(); } catch (e) { fail(e); }
        } }, 'Revoke'))) : h('div', { style: 'color:var(--faint);padding:6px 0 10px' }, 'No devices yet.')),
      h('button', { class: 'btn', style: 'margin-top:10px', onclick: async () => {
        const name = await prompt2('New app password', { label: 'Device or app name', value: 'ncSolid', ok: 'Create' });
        if (!name) return;
        try {
          const r = await api('POST', '/api/devices/create', { name });
          await modal((m, close) => m.append(h('h2', {}, 'App password created'), h('p', {}, 'Copy it now; it won’t be shown again.'),
            h('div', { class: 'kv' }, h('span', {}, 'Username'), h('div', { class: 'mono' }, S.me.username), h('span', {}, 'Password'), h('div', { class: 'mono' }, r.token)),
            h('div', { class: 'actions' }, h('button', { class: 'btn', onclick: () => { navigator.clipboard.writeText(r.token); toast('Copied'); } }, 'Copy'), h('button', { class: 'btn primary', onclick: () => close() }, 'Done'))));
          loadDevices();
        } catch (e) { fail(e); }
      } }, icon('plus'), 'Create app password'));
    cards.sessions.replaceChildren(h('h3', {}, icon('monitor'), 'Browser sessions'), h('p', {}, 'Browsers currently signed in to your account.'),
      h('div', { class: 'items' }, d.sessions.map((s) => h('div', { class: 'it' },
        h('div', { class: 't' }, h('b', {}, uaName(s.user_agent), s.current ? h('span', { class: 'pill on', style: 'margin-left:8px' }, 'This browser') : ''), h('small', {}, `Signed in ${fmtDate(s.created_at * 1000)} · active ${fmtDate(s.last_seen * 1000)}`)),
        !s.current && h('button', { class: 'btn', onclick: async () => { try { await api('POST', '/api/devices/revoke', { session_id: s.id }); loadDevices(); } catch (e) { fail(e); } } }, 'Sign out')))));
  }
  loadDevices();
  return [toolbar, content];
}
function uaName(ua) {
  if (!ua) return 'Unknown browser';
  const b = /Edg\//.test(ua) ? 'Edge' : /Firefox\//.test(ua) ? 'Firefox' : /Chrome\//.test(ua) ? 'Chrome' : /Safari\//.test(ua) ? 'Safari' : 'Browser';
  const os = /iPhone|iPad/.test(ua) ? 'iOS' : /Android/.test(ua) ? 'Android' : /Mac OS X/.test(ua) ? 'macOS' : /Windows/.test(ua) ? 'Windows' : /Linux/.test(ua) ? 'Linux' : '';
  return os ? `${b} on ${os}` : b;
}

/* ================= keyboard ================= */

addEventListener('keydown', (e) => {
  if (!S.me || $('.overlay') || $('.viewer')) return;
  const typing = /INPUT|TEXTAREA/.test(document.activeElement.tagName);
  if (e.key === '/' && !typing) { e.preventDefault(); const si = $('#search'); si && si.focus(); return; }
  if (typing || S.route !== 'files') return;
  if (e.key === 'Escape') { closeMenu(); if (S.sel.size) { S.sel.clear(); renderMain(); } }
  else if ((e.key === 'Delete' || (e.key === 'Backspace' && e.metaKey)) && S.sel.size) { e.preventDefault(); remove(selected()); }
  else if ((e.metaKey || e.ctrlKey) && e.key === 'a') { e.preventDefault(); S.entries.forEach((x) => S.sel.add(x.path)); renderMain(); }
  else if (e.key === 'F2' && S.sel.size === 1) rename(selected()[0]);
  else if (e.key === 'Backspace' && !S.sel.size && S.path && !S.query) go(parentOf(S.path));
});

/* ================= boot ================= */

async function boot() {
  try { S.me = await api('GET', '/api/me'); } catch { return showLogin(); }
  renderShell();
  loadShares().then(() => { if (S.route === 'files') renderMain(); });
  await route();
}
boot();
