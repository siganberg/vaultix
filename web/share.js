'use strict';
/* Public share page: /s/<token>. Viewing links and file requests. */

const $ = (s, el = document) => el.querySelector(s);
function h(tag, attrs, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v == null || v === false) continue;
    if (k.startsWith('on')) el.addEventListener(k.slice(2).toLowerCase(), v);
    else if (k === 'class') el.className = v;
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
  lock: '<rect x="4" y="11" width="16" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
  x: '<path d="M18 6 6 18M6 6l12 12"/>',
  check: '<path d="m5 12 5 5L20 7"/>',
  left: '<path d="m15 18-6-6 6-6"/>',
  right: '<path d="m9 18 6-6-6-6"/>',
  alert: '<circle cx="12" cy="12" r="9"/><path d="M12 8v5M12 16h.01"/>',
  eye: '<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7z"/><circle cx="12" cy="12" r="3"/>',
};
const icon = (n) => {
  const s = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  s.setAttribute('viewBox', '0 0 24 24');
  s.setAttribute('class', 'i');
  s.innerHTML = P[n] || P.file;
  return s;
};
const EXT = {
  image: 'jpg jpeg png gif webp avif bmp ico heic svg tif tiff',
  video: 'mp4 webm mov m4v mkv avi',
  audio: 'mp3 wav ogg m4a flac aac opus',
  pdf: 'pdf',
  archive: 'zip tar gz tgz bz2 xz 7z rar dmg iso',
  model: 'stl obj 3mf step stp iges igs f3d dxf scad blend fbx gltf glb',
  code: 'js ts tsx jsx json rs py go c h cpp hpp cs java kt swift rb php sh yml yaml toml xml html css scss sql ini conf cfg lua gcode nc ngc tap',
};
const TEXT_EXT = new Set('txt md log csv tsv json js ts tsx jsx rs py go c h cpp hpp cs java kt swift rb php sh yml yaml toml xml html css scss sql ini conf cfg lua gcode nc ngc tap'.split(' '));
const ext = (n) => { const i = n.lastIndexOf('.'); return i > 0 ? n.slice(i + 1).toLowerCase() : ''; };
function kind(e) {
  if (e.is_dir) return 'folder';
  const x = ext(e.name);
  for (const k of ['image', 'video', 'audio', 'pdf', 'archive', 'model', 'code']) if (EXT[k].split(' ').includes(x)) return k;
  return 'doc';
}
const ficon = (e) => { const k = kind(e); return h('div', { class: 'ficon ' + k }, icon(k === 'doc' ? 'file' : k)); };
function fmtSize(b) {
  if (b == null) return '';
  if (b < 1024) return b + ' B';
  const u = ['KB', 'MB', 'GB', 'TB']; let i = -1;
  do { b /= 1024; i++; } while (b >= 1024 && i < u.length - 1);
  return (b < 10 ? b.toFixed(1) : Math.round(b)) + ' ' + u[i];
}
const fmtDate = (ms) => ms ? new Date(ms).toLocaleDateString([], { year: 'numeric', month: 'short', day: 'numeric' }) : '';
const baseName = (p) => p.slice(p.lastIndexOf('/') + 1);

const TOKEN = (location.pathname.match(/^\/s\/([0-9a-fA-F]{32})/) || [])[1] || '';
const BASE = '/api/public/' + TOKEN;
const fileUrl = (p, dl) => BASE + '/file?path=' + encodeURIComponent(p || '') + (dl ? '&download=1' : '');

async function api(method, url, body) {
  const r = await fetch(url, { method, headers: { 'x-vaultix': '1', ...(body ? { 'content-type': 'application/json' } : {}) }, body: body ? JSON.stringify(body) : undefined });
  let d = null; try { d = await r.json(); } catch {}
  if (!r.ok) { const e = new Error((d && d.error) || r.statusText); e.status = r.status; throw e; }
  return d;
}

const root = $('#root');
let INFO = null;

function frame(...content) {
  const top = h('header', { class: 'sp-top' },
    h('a', { class: 'brand', href: '/', title: 'Vaultix' }, h('img', { src: '/icon.svg', alt: '' }), 'Vaultix'),
    h('div', { class: 'sp-right' },
      INFO && INFO.owner ? h('div', { class: 'sp-by' }, h('div', { class: 'avatar' }, INFO.owner[0]), h('span', {}, 'Shared by ', h('b', {}, INFO.owner))) : '',
      VXTheme.button()));
  root.replaceChildren(top, h('main', { class: 'sp-main' }, ...content), h('footer', { class: 'sp-foot' }, 'Shared securely with Vaultix'));
}

function message(ic, title, text) {
  frame(h('div', { class: 'sp-card sp-center' }, h('div', { class: 'sp-badge' + (ic === 'alert' ? ' bad' : '') }, icon(ic)), h('h1', {}, title), h('p', {}, text)));
}

async function boot() {
  if (!TOKEN) return message('alert', 'Link unavailable', 'This link isn’t valid.');
  try { INFO = await api('GET', BASE); } catch (e) {
    return message('alert', e.status === 410 ? 'This link has expired' : 'Link unavailable', e.message);
  }
  if (INFO.locked) return locked();
  document.title = `${INFO.kind === 'upload' ? 'Send files to ' + INFO.owner : INFO.name} · Vaultix`;
  if (INFO.kind === 'upload') return uploadView();
  if (INFO.is_dir) return folderView(decodeURIComponent(location.hash.slice(1)));
  return fileView();
}
addEventListener('hashchange', () => { if (INFO && !INFO.locked && INFO.is_dir && INFO.kind === 'link') folderView(decodeURIComponent(location.hash.slice(1))); });

/* ---------- password ---------- */

function locked() {
  const pw = h('input', { class: 'input', type: 'password', placeholder: 'Password', autocomplete: 'current-password' });
  const err = h('div', { class: 'err' });
  const btn = h('button', { class: 'btn primary', style: 'width:100%;justify-content:center;height:42px' }, 'Unlock');
  const submit = async (e) => {
    e.preventDefault(); err.textContent = ''; btn.disabled = true;
    try { await api('POST', BASE + '/unlock', { password: pw.value }); await boot(); }
    catch (ex) { err.textContent = ex.message; pw.select(); }
    finally { btn.disabled = false; }
  };
  frame(h('form', { class: 'sp-card sp-center sp-narrow', onsubmit: submit },
    h('div', { class: 'sp-badge' }, icon('lock')), h('h1', {}, 'Password required'),
    h('p', {}, `${INFO.owner} protected this ${INFO.kind === 'upload' ? 'upload link' : 'link'} with a password.`),
    pw, err, btn));
  pw.focus();
}

/* ---------- single file ---------- */

function previewEl(e, url) {
  const k = kind(e), x = ext(e.name);
  if (k === 'image' && !['heic', 'tif', 'tiff'].includes(x)) return h('img', { src: url, alt: e.name });
  if (k === 'video' && !['mkv', 'avi'].includes(x)) return h('video', { src: url, controls: true, playsinline: true, preload: 'metadata' });
  if (k === 'audio') return h('audio', { src: url, controls: true });
  if (k === 'pdf') return h('iframe', { src: url, title: e.name });
  if ((TEXT_EXT.has(x) || !e.name.includes('.')) && e.size < 2e6) {
    const pre = h('pre', {}, 'Loading…');
    fetch(url).then((r) => r.text()).then((t) => { pre.textContent = t; }).catch(() => { pre.textContent = 'Could not load file.'; });
    return pre;
  }
  return null;
}

function fileView() {
  const e = { name: INFO.name, is_dir: false, size: INFO.size };
  const pv = previewEl(e, fileUrl(''));
  frame(h('div', { class: 'sp-card sp-file' },
    pv ? h('div', { class: 'sp-preview k-' + kind(e) }, pv) : h('div', { class: 'sp-preview none' }, h('div', { class: 'ficon ' + kind(e) }, icon(kind(e) === 'doc' ? 'file' : kind(e)))),
    h('div', { class: 'sp-meta' },
      h('div', { class: 'sp-name' }, h('h1', {}, INFO.name), h('p', {}, `${fmtSize(INFO.size)} · ${fmtDate(INFO.mtime)}`)),
      INFO.allow_download
        ? h('a', { class: 'btn primary', href: fileUrl('', true) }, icon('download'), 'Download')
        : h('span', { class: 'pill' }, icon('eye'), ' View only'))));
}

/* ---------- folder ---------- */

async function folderView(sub) {
  let entries = [];
  try { entries = (await api('GET', BASE + '/list?path=' + encodeURIComponent(sub))).entries; }
  catch (e) { return message('alert', 'Folder unavailable', e.message); }
  entries.sort((a, b) => (a.is_dir !== b.is_dir ? (a.is_dir ? -1 : 1) : a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' })));
  const parts = sub ? sub.split('/') : [];
  const crumbs = h('nav', { class: 'crumbs' },
    h('button', { onclick: () => { location.hash = ''; } }, INFO.name),
    parts.map((p, i) => [h('span', { class: 'sep' }, '/'), h('button', { onclick: () => { location.hash = encodeURIComponent(parts.slice(0, i + 1).join('/')); } }, p)]));
  const files = entries.filter((e) => !e.is_dir);
  const tbody = h('tbody');
  for (const e of entries) {
    tbody.append(h('tr', { class: 'row', onclick: () => (e.is_dir ? (location.hash = encodeURIComponent(e.path)) : viewer(files, files.indexOf(e))) },
      h('td', { class: 'name' }, h('div', { class: 'nm' }, ficon(e), h('span', { title: e.name }, e.name))),
      h('td', { class: 'meta hide-sm' }, e.is_dir ? '—' : fmtSize(e.size)),
      h('td', { class: 'meta hide-sm' }, fmtDate(e.mtime)),
      h('td', { class: 'act' }, !e.is_dir && INFO.allow_download && h('a', { class: 'btn ghost icon', href: fileUrl(e.path, true), title: 'Download', onclick: (ev) => ev.stopPropagation() }, icon('download')))));
  }
  frame(h('div', { class: 'sp-card sp-folder' },
    h('div', { class: 'sp-fhead' },
      h('div', { class: 'ficon folder', style: 'width:44px;height:44px;border-radius:12px' }, icon('folder')),
      h('div', { class: 'sp-name' }, h('h1', {}, INFO.name), h('p', {}, `${entries.length} item${entries.length === 1 ? '' : 's'}`)),
      INFO.allow_download
        ? h('a', { class: 'btn primary', href: BASE + '/zip' + (sub ? '?paths=' + encodeURIComponent(JSON.stringify([sub])) : '') }, icon('download'), sub ? 'Download folder' : 'Download all')
        : h('span', { class: 'pill' }, icon('eye'), ' View only')),
    parts.length ? crumbs : '',
    entries.length
      ? h('table', { class: 'list' }, h('thead', {}, h('tr', {}, h('th', {}, 'Name'), h('th', { class: 'hide-sm' }, 'Size'), h('th', { class: 'hide-sm' }, 'Modified'), h('th'))), tbody)
      : h('div', { class: 'empty' }, 'This folder is empty.')));
}

function viewer(files, idx) {
  const body = h('div', { class: 'vb' });
  const title = h('b');
  const dl = h('a', { class: 'btn' }, icon('download'), 'Download');
  const v = h('div', { class: 'viewer' }, h('div', { class: 'vh' }, title, INFO.allow_download ? dl : '', h('button', { class: 'btn icon', onclick: () => close() }, icon('x'))), body);
  const prev = h('button', { class: 'nav-btn l', onclick: () => step(-1) }, icon('left'));
  const next = h('button', { class: 'nav-btn r', onclick: () => step(1) }, icon('right'));
  const close = () => { v.remove(); removeEventListener('keydown', key, true); };
  const key = (e) => { if (e.key === 'Escape') close(); else if (e.key === 'ArrowLeft') step(-1); else if (e.key === 'ArrowRight') step(1); };
  const step = (d) => { const n = idx + d; if (n >= 0 && n < files.length) { idx = n; show(); } };
  function show() {
    const f = files[idx];
    title.textContent = f.name;
    dl.href = fileUrl(f.path, true);
    const el = previewEl(f, fileUrl(f.path)) || h('div', { class: 'nopv' }, h('div', { class: 'ficon ' + kind(f), style: 'width:72px;height:72px;margin:0 auto 14px;border-radius:18px' }, icon(kind(f) === 'doc' ? 'file' : kind(f))),
      h('div', { style: 'font-weight:600;color:#fff' }, f.name), h('div', {}, fmtSize(f.size) + ' · No preview available'));
    body.replaceChildren(el, idx > 0 ? prev : '', idx < files.length - 1 ? next : '');
  }
  addEventListener('keydown', key, true);
  document.body.append(v);
  show();
}

/* ---------- file request ---------- */

function uploadView() {
  const items = [];
  let active = 0;
  const queue = [];
  const nameIn = h('input', { class: 'input', placeholder: 'Your name (optional)', maxlength: '80', autocomplete: 'name' });
  try { nameIn.value = localStorage.getItem('vx.uploader') || ''; } catch {}
  nameIn.addEventListener('change', () => { try { localStorage.setItem('vx.uploader', nameIn.value); } catch {} });
  const list = h('div', { class: 'sp-ulist' });
  const summary = h('div', { class: 'sp-usum' });
  const pick = h('input', { type: 'file', multiple: true, class: 'hidden' });
  pick.addEventListener('change', () => { add([...pick.files]); pick.value = ''; });
  const drop = h('div', { class: 'sp-drop', onclick: () => pick.click() },
    h('div', { class: 'sp-badge' }, icon('upload')),
    h('b', {}, 'Drop files here'), h('span', {}, 'or click to choose files'),
    INFO.max_size ? h('small', {}, `Up to ${fmtSize(INFO.max_size)} per file`) : '');
  for (const ev of ['dragenter', 'dragover']) drop.addEventListener(ev, (e) => { e.preventDefault(); drop.classList.add('over'); });
  for (const ev of ['dragleave', 'drop']) drop.addEventListener(ev, () => drop.classList.remove('over'));
  drop.addEventListener('drop', (e) => { e.preventDefault(); add([...e.dataTransfer.files]); });
  addEventListener('dragover', (e) => e.preventDefault());
  addEventListener('drop', (e) => e.preventDefault());

  function add(files) {
    for (const f of files) {
      const it = { f, loaded: 0, state: 'queued', el: h('div', { class: 'ui' }) };
      if (INFO.max_size && f.size > INFO.max_size) { it.state = 'fail'; it.err = `Larger than ${fmtSize(INFO.max_size)}`; }
      else queue.push(it);
      items.push(it); list.prepend(it.el); draw(it);
    }
    pump(); sum();
  }
  function draw(it) {
    const pct = it.f.size ? (it.loaded / it.f.size) * 100 : it.state === 'done' ? 100 : 0;
    it.el.className = 'ui ' + it.state;
    it.el.replaceChildren(h('div', { class: 'r' }, h('b', { title: it.f.name }, it.saved || it.f.name),
      h('small', {}, it.state === 'fail' ? it.err : it.state === 'done' ? '✓ Sent' : it.state === 'queued' ? 'Waiting' : Math.floor(pct) + '%')),
      h('div', { class: 'bar' }, h('i', { style: `width:${it.state === 'fail' ? 100 : pct}%` })));
  }
  function sum() {
    const done = items.filter((i) => i.state === 'done').length, fail = items.filter((i) => i.state === 'fail').length;
    const busy = active + queue.length;
    summary.textContent = busy ? `Sending ${done + 1 > items.length ? items.length : done + 1} of ${items.length}…` : items.length ? `${done} file${done === 1 ? '' : 's'} sent${fail ? ` · ${fail} failed` : ''}. ${done ? INFO.owner + ' will find them in their drive.' : ''}` : '';
  }
  function pump() {
    while (active < 3 && queue.length) {
      const it = queue.shift(); active++; it.state = 'up';
      const xhr = new XMLHttpRequest();
      xhr.open('PUT', BASE + '/upload?name=' + encodeURIComponent(it.f.name) + (nameIn.value.trim() ? '&uploader=' + encodeURIComponent(nameIn.value.trim()) : ''));
      xhr.setRequestHeader('x-vaultix', '1');
      xhr.upload.onprogress = (e) => { it.loaded = e.loaded; draw(it); };
      xhr.onloadend = () => {
        active--;
        let d = null; try { d = JSON.parse(xhr.responseText); } catch {}
        if (xhr.status >= 200 && xhr.status < 300) { it.state = 'done'; it.loaded = it.f.size; it.saved = d && d.name; }
        else { it.state = 'fail'; it.err = (d && d.error) || 'Upload failed'; }
        draw(it); sum(); pump();
      };
      xhr.send(it.f);
      draw(it);
    }
    sum();
  }

  frame(h('div', { class: 'sp-card sp-upload' },
    h('div', { class: 'sp-uhead' },
      h('div', { class: 'avatar', style: 'width:44px;height:44px;font-size:18px' }, INFO.owner[0]),
      h('div', {}, h('h1', {}, `${INFO.owner} is requesting files`), h('p', {}, 'Files you send go straight to their Vaultix drive. You won’t see anything else in it.'))),
    INFO.note ? h('blockquote', { class: 'sp-note' }, INFO.note) : '',
    nameIn, drop, pick, summary, list));
}

boot();
