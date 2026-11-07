// Full-screen file viewer shared by the app and public share pages.
// VXViewer.open({ files, index, url(f), meta(f), download(f) | downloadHref(f) })
(() => {
  const NS = 'http://www.w3.org/2000/svg';
  const IC = {
    x: '<path d="M18 6 6 18M6 6l12 12"/>',
    left: '<path d="m15 18-6-6 6-6"/>',
    right: '<path d="m9 18 6-6-6-6"/>',
    download: '<path d="M12 4v12M7 11l5 5 5-5"/><path d="M20 16v3a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-3"/>',
    open: '<path d="M15 3h6v6M10 14 21 3M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
    plus: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5M8 11h6M11 8v6"/>',
    minus: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5M8 11h6"/>',
    fit: '<path d="M4 9V5a1 1 0 0 1 1-1h4M15 4h4a1 1 0 0 1 1 1v4M20 15v4a1 1 0 0 1-1 1h-4M9 20H5a1 1 0 0 1-1-1v-4"/>',
    wrap: '<path d="M3 6h18M3 12h15a3 3 0 0 1 0 6h-4M3 18h7"/><path d="m16 16-2 2 2 2"/>',
    reset: '<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/>',
  };
  function h(tag, attrs, ...kids) {
    const el = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs || {})) {
      if (v == null || v === false) continue;
      if (k.startsWith('on')) el.addEventListener(k.slice(2).toLowerCase(), v);
      else if (k === 'class') el.className = v;
      else el.setAttribute(k, v === true ? '' : v);
    }
    for (const c of kids.flat(Infinity)) if (c != null && c !== false && c !== '') el.append(c instanceof Node ? c : String(c));
    return el;
  }
  const icon = (n) => { const s = document.createElementNS(NS, 'svg'); s.setAttribute('viewBox', '0 0 24 24'); s.setAttribute('class', 'i'); s.innerHTML = IC[n]; return s; };
  const ext = (n) => { const i = n.lastIndexOf('.'); return i > 0 ? n.slice(i + 1).toLowerCase() : ''; };
  const TEXT = new Set('txt md markdown log csv tsv json js mjs ts tsx jsx rs py go c h cpp hpp cs java kt swift rb php sh zsh bash yml yaml toml xml html css scss sql ini conf cfg env lua gcode nc ngc tap gc cnc gitignore dockerfile makefile svg'.split(' '));
  const GCODE = new Set('gcode nc ngc tap gc cnc'.split(' '));
  const MESH = new Set(['stl', 'obj']);
  const nf = (n) => n.toLocaleString();

  function open(o) {
    const files = o.files;
    let idx = Math.max(0, o.index || 0);
    let cleanup = null, keyHook = null;

    const title = h('b');
    const sub = h('small');
    const fic = h('div', { class: 'vfi' });
    const tools = h('div', { class: 'vtools' });
    const openBtn = h('a', { class: 'btn icon', target: '_blank', rel: 'noopener', title: 'Open in new tab' }, icon('open'));
    const dl = o.download || o.downloadHref ? h('a', { class: 'btn', title: 'Download' }, icon('download'), h('span', { class: 'lbl' }, 'Download')) : null;
    const body = h('div', { class: 'vb' });
    const prev = h('button', { class: 'nav-btn l', title: 'Previous (←)', onclick: () => step(-1) }, icon('left'));
    const next = h('button', { class: 'nav-btn r', title: 'Next (→)', onclick: () => step(1) }, icon('right'));
    const v = h('div', { class: 'viewer', role: 'dialog', 'aria-modal': 'true' },
      h('div', { class: 'vh' }, fic, h('div', { class: 'vt' }, title, sub), tools, openBtn, dl,
        h('button', { class: 'btn icon', title: 'Close (Esc)', onclick: () => close() }, icon('x'))),
      body);
    if (dl && o.download) dl.addEventListener('click', (e) => { e.preventDefault(); o.download(files[idx]); });

    function close() {
      if (cleanup) cleanup();
      v.remove(); removeEventListener('keydown', key, true);
      document.documentElement.classList.remove('viewer-open');
    }
    function key(e) {
      if (e.target.closest && e.target.closest('input, textarea')) return;
      if (keyHook && keyHook(e)) { e.preventDefault(); return; }
      if (e.key === 'Escape') { e.stopPropagation(); close(); }
      else if (e.key === 'ArrowLeft') step(-1);
      else if (e.key === 'ArrowRight') step(1);
    }
    function step(d) { const n = idx + d; if (n >= 0 && n < files.length) { idx = n; show(); } }

    function show() {
      if (cleanup) { cleanup(); cleanup = null; }
      keyHook = null;
      tools.replaceChildren();
      const f = files[idx], x = ext(f.name), url = o.url(f);
      title.textContent = f.name; title.title = f.name;
      sub.textContent = [o.meta ? o.meta(f) : '', files.length > 1 ? `${idx + 1} of ${files.length}` : ''].filter(Boolean).join('  ·  ');
      fic.replaceChildren(FileIcons.el(f.name, false));
      openBtn.href = url;
      if (dl && o.downloadHref) dl.href = o.downloadHref(f);
      const t = FileIcons.type(f.name, false).t;
      const stage = h('div', { class: 'vstage' });
      body.replaceChildren(stage, idx > 0 ? prev : '', idx < files.length - 1 ? next : '');
      const ctx = { f, x, url, stage, tools, fail: () => noPreview(f, stage), setCleanup: (fn) => { cleanup = fn; }, setKeys: (fn) => { keyHook = fn; } };
      if (t === 'image') imageView(ctx);
      else if (t === 'video') mediaView(ctx, 'video');
      else if (t === 'audio') audioView(ctx);
      else if (t === 'pdf') stage.append(h('iframe', { src: url, title: f.name, class: 'vpdf' }));
      else if (MESH.has(x) && f.size < 200e6) meshView(ctx);
      else if (GCODE.has(x) && f.size < 60e6) textView(ctx, 'gcode');
      else if ((TEXT.has(x) || !f.name.includes('.')) && x !== 'svg') textView(ctx, x === 'md' || x === 'markdown' ? 'md' : x === 'csv' || x === 'tsv' ? 'csv' : x === 'json' ? 'json' : 'code');
      else noPreview(f, stage);
      // Warm the neighbours so arrowing through photos feels instant.
      for (const j of [idx + 1, idx - 1]) { const n = files[j]; if (n && FileIcons.type(n.name).t === 'image' && n.size < 30e6) new Image().src = o.url(n); }
    }

    function noPreview(f, stage) {
      stage.replaceChildren(h('div', { class: 'vnone' },
        h('div', { class: 'vnone-ic' }, FileIcons.el(f.name, false, true)),
        h('div', { class: 'vnone-name' }, f.name),
        h('div', { class: 'vnone-sub' }, (o.meta ? o.meta(f) + ' · ' : '') + 'No preview for this file type'),
        dl ? h('button', { class: 'btn primary', onclick: () => dl.click() }, icon('download'), 'Download') : ''));
    }

    /* ---------- images: zoom & pan ---------- */
    function imageView({ f, url, stage, tools, fail, setKeys }) {
      const spin = h('div', { class: 'vspin' });
      const img = h('img', { src: url, alt: f.name, draggable: 'false' });
      const box = h('div', { class: 'vimg loading' }, img, spin);
      stage.append(box);
      let s = 1, tx = 0, ty = 0;
      const pct = h('button', { class: 'btn vpct', title: 'Fit (0)', onclick: () => set(1, 0, 0) }, '100%');
      tools.append(h('button', { class: 'btn icon', title: 'Zoom out (−)', onclick: () => zoomBy(1 / 1.5) }, icon('minus')), pct,
        h('button', { class: 'btn icon', title: 'Zoom in (+)', onclick: () => zoomBy(1.5) }, icon('plus')));
      img.onload = () => { box.classList.remove('loading'); spin.remove(); label(); };
      img.onerror = fail;
      function label() {
        const nat = img.naturalWidth && img.clientWidth ? (img.clientWidth * s) / img.naturalWidth : s;
        pct.textContent = Math.round(nat * 100) + '%';
      }
      function set(ns, nx, ny, anim = true) {
        s = Math.min(16, Math.max(1, ns));
        if (s === 1) { nx = 0; ny = 0; }
        tx = nx; ty = ny;
        img.style.transition = anim ? 'transform .18s ease' : 'none';
        img.style.transform = `translate(${tx}px, ${ty}px) scale(${s})`;
        box.classList.toggle('zoomed', s > 1);
        label();
      }
      function zoomAt(ns, px, py, anim) {
        ns = Math.min(16, Math.max(1, ns));
        const r = box.getBoundingClientRect();
        const cx = px - r.left - r.width / 2, cy = py - r.top - r.height / 2;
        set(ns, cx - (ns * (cx - tx)) / s, cy - (ns * (cy - ty)) / s, anim);
      }
      function zoomBy(k) { const r = box.getBoundingClientRect(); zoomAt(s * k, r.left + r.width / 2, r.top + r.height / 2, true); }
      box.addEventListener('wheel', (e) => { e.preventDefault(); zoomAt(s * Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0022)), e.clientX, e.clientY, false); }, { passive: false });
      box.addEventListener('dblclick', (e) => (s > 1 ? set(1, 0, 0) : zoomAt(2.5, e.clientX, e.clientY, true)));
      const pts = new Map();
      let drag = null, pinch = null;
      box.addEventListener('pointerdown', (e) => {
        box.setPointerCapture(e.pointerId);
        pts.set(e.pointerId, { x: e.clientX, y: e.clientY });
        if (pts.size === 2) {
          const [a, b] = [...pts.values()];
          pinch = { d: Math.hypot(a.x - b.x, a.y - b.y), s }; drag = null;
        } else drag = { x: e.clientX, y: e.clientY, tx, ty, t: performance.now(), touch: e.pointerType === 'touch' };
      });
      box.addEventListener('pointermove', (e) => {
        if (!pts.has(e.pointerId)) return;
        pts.set(e.pointerId, { x: e.clientX, y: e.clientY });
        if (pinch && pts.size === 2) {
          const [a, b] = [...pts.values()];
          zoomAt(pinch.s * (Math.hypot(a.x - b.x, a.y - b.y) / pinch.d), (a.x + b.x) / 2, (a.y + b.y) / 2, false);
        } else if (drag && s > 1) set(s, drag.tx + e.clientX - drag.x, drag.ty + e.clientY - drag.y, false);
      });
      const up = (e) => {
        if (drag && s === 1 && drag.touch && pts.size === 1) {
          const dx = e.clientX - drag.x;
          if (Math.abs(dx) > 60 && Math.abs(e.clientY - drag.y) < 80) step(dx < 0 ? 1 : -1);
        }
        pts.delete(e.pointerId);
        if (pts.size < 2) pinch = null;
        if (!pts.size) drag = null;
      };
      box.addEventListener('pointerup', up);
      box.addEventListener('pointercancel', up);
      setKeys((e) => {
        if (e.key === '+' || e.key === '=') { zoomBy(1.5); return true; }
        if (e.key === '-' || e.key === '_') { zoomBy(1 / 1.5); return true; }
        if (e.key === '0') { set(1, 0, 0); return true; }
        if (s > 1 && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) return true;
        return false;
      });
    }

    /* ---------- video / audio ---------- */
    function mediaView({ url, stage, fail }) {
      const el = h('video', { src: url, controls: true, autoplay: true, playsinline: true, class: 'vvideo' });
      el.onerror = fail;
      stage.append(el);
    }
    function audioView({ f, url, stage, fail }) {
      const el = h('audio', { src: url, controls: true, autoplay: true });
      el.onerror = fail;
      stage.append(h('div', { class: 'vaudio' }, h('div', { class: 'vaudio-art' }, FileIcons.el(f.name, false, true)),
        h('div', { class: 'vnone-name' }, f.name.replace(/\.[^.]+$/, '')), h('div', { class: 'vnone-sub' }, o.meta ? o.meta(f) : ''), el));
    }

    /* ---------- text, code, markdown, csv, g-code ---------- */
    function textView(ctx, mode) {
      const { f, url, stage, tools } = ctx;
      const LIMIT = mode === 'gcode' ? 60e6 : 2e6;
      const partial = f.size > LIMIT;
      const spin = h('div', { class: 'vspin' });
      stage.append(spin);
      const opts = partial ? { headers: { range: `bytes=0-${LIMIT - 1}` } } : {};
      let alive = true;
      ctx.setCleanup(() => { alive = false; if (ctx._c) ctx._c(); });
      fetch(url, opts).then((r) => { if (!r.ok) throw new Error(); return r.text(); }).then((txt) => {
        if (!alive) return;
        spin.remove();
        const note = partial ? h('div', { class: 'vnote' }, `Showing the first ${Math.round(LIMIT / 1e6)} MB of ${(f.size / 1e6).toFixed(1)} MB.`) : '';
        const views = {};
        if (mode === 'md') views.Preview = () => h('div', { class: 'vdoc' }, h('article', { class: 'md' }, markdown(txt)));
        if (mode === 'csv') views.Table = () => csvTable(txt, ext(f.name) === 'tsv' ? '\t' : null);
        if (mode === 'gcode') views.Toolpath = () => { const el = h('div', { class: 'v3d' }); setTimeout(() => { if (alive) ctx._c = gcodeView(el, txt); }); return el; };
        let src = txt;
        if (mode === 'json') { try { src = JSON.stringify(JSON.parse(txt), null, 2); } catch {} }
        views[Object.keys(views).length ? 'Source' : ''] = () => codeView(src, mode === 'gcode');
        const names = Object.keys(views);
        const holder = h('div', { class: 'vtext' });
        stage.append(holder, note);
        function pick(n) {
          if (ctx._c) { ctx._c(); ctx._c = null; }
          holder.replaceChildren(views[n]());
          if (seg) seg.querySelectorAll('button').forEach((b) => b.classList.toggle('on', b.textContent === n));
        }
        const seg = names.length > 1 ? h('div', { class: 'seg seg-text' }, names.map((n) => h('button', { onclick: () => pick(n) }, n))) : null;
        if (seg) tools.append(seg);
        tools.append(h('button', { class: 'btn icon', title: 'Wrap lines', onclick: (e) => { holder.classList.toggle('wrap'); e.currentTarget.classList.toggle('on'); } }, icon('wrap')));
        pick(names[0]);
      }).catch(() => alive && ctx.fail());
    }

    function codeView(txt, gcode) {
      const lines = txt.split(/\r?\n/);
      if (lines.length > 1 && lines[lines.length - 1] === '') lines.pop();
      const gut = h('pre', { class: 'gut' }, lines.map((_, i) => i + 1).join('\n'));
      const code = h('pre', { class: 'src' });
      if (gcode && lines.length < 200000) {
        // Light G-code highlighting: comments, G/M words, axis words.
        const frag = document.createDocumentFragment();
        const re = /(\(.*?\)|;.*$)|([GM]\d+(?:\.\d+)?)|([XYZIJKRABC][-+]?\d*\.?\d+)|([FST][-+]?\d*\.?\d+)|(N\d+)/gim;
        for (const ln of lines) {
          let last = 0, m;
          re.lastIndex = 0;
          while ((m = re.exec(ln))) {
            if (m.index > last) frag.append(ln.slice(last, m.index));
            const cls = m[1] ? 'c' : m[2] ? 'k' : m[3] ? 'a' : m[4] ? 'n' : 'l';
            frag.append(h('span', { class: 'g' + cls }, m[0]));
            last = m.index + m[0].length;
          }
          frag.append(ln.slice(last) + '\n');
        }
        code.append(frag);
      } else code.textContent = lines.join('\n');
      return h('div', { class: 'vcode' }, gut, code);
    }

    function csvTable(txt, delim) {
      delim = delim || ((txt.split('\n', 1)[0].match(/;/g) || []).length > (txt.split('\n', 1)[0].match(/,/g) || []).length ? ';' : ',');
      const rows = [];
      let row = [], cell = '', q = false;
      for (let i = 0; i < txt.length && rows.length < 2001; i++) {
        const c = txt[i];
        if (q) { if (c === '"') { if (txt[i + 1] === '"') { cell += '"'; i++; } else q = false; } else cell += c; }
        else if (c === '"') q = true;
        else if (c === delim) { row.push(cell); cell = ''; }
        else if (c === '\n' || c === '\r') { if (c === '\r' && txt[i + 1] === '\n') i++; row.push(cell); rows.push(row); row = []; cell = ''; }
        else cell += c;
      }
      if (cell || row.length) { row.push(cell); rows.push(row); }
      const [head, ...rest] = rows;
      const more = rest.length > 2000;
      return h('div', { class: 'vdoc' }, h('div', { class: 'vcsv' }, h('table', {},
        h('thead', {}, h('tr', {}, h('th', {}, ''), (head || []).map((c) => h('th', {}, c)))),
        h('tbody', {}, rest.slice(0, 2000).map((r, i) => h('tr', {}, h('td', { class: 'rn' }, i + 1), r.map((c) => h('td', {}, c))))))),
        more ? h('div', { class: 'vnote' }, 'Showing the first 2,000 rows.') : '');
    }

    // Small, safe Markdown → DOM (no HTML passthrough).
    function markdown(src) {
      const out = document.createDocumentFragment();
      const lines = src.replace(/\r\n?/g, '\n').split('\n');
      let i = 0;
      const isBlockStart = (l) => /^(#{1,6}\s|```|>\s?|\s*([-*+]|\d+[.)])\s|\s*(---|\*\*\*|___)\s*$|\|)/.test(l);
      while (i < lines.length) {
        const l = lines[i];
        if (!l.trim()) { i++; continue; }
        let m;
        if ((m = l.match(/^```\s*(\S*)/))) {
          const buf = []; i++;
          while (i < lines.length && !/^```/.test(lines[i])) buf.push(lines[i++]);
          i++;
          out.append(h('pre', {}, h('code', {}, buf.join('\n'))));
        } else if ((m = l.match(/^(#{1,6})\s+(.*?)\s*#*$/))) {
          out.append(h('h' + m[1].length, {}, inline(m[2]))); i++;
        } else if (/^\s*(---|\*\*\*|___)\s*$/.test(l)) { out.append(h('hr')); i++; }
        else if (/^>/.test(l)) {
          const buf = [];
          while (i < lines.length && /^>/.test(lines[i])) buf.push(lines[i++].replace(/^>\s?/, ''));
          out.append(h('blockquote', {}, markdown(buf.join('\n'))));
        } else if (/^\s*([-*+]|\d+[.)])\s/.test(l)) {
          const ordered = /^\s*\d/.test(l);
          const list = h(ordered ? 'ol' : 'ul');
          while (i < lines.length && /^\s*([-*+]|\d+[.)])\s/.test(lines[i])) {
            let t = lines[i++].replace(/^\s*([-*+]|\d+[.)])\s+/, '');
            const task = t.match(/^\[([ xX])\]\s+/);
            const li = h('li', task ? { class: 'task' } : {});
            if (task) { li.append(h('input', { type: 'checkbox', disabled: true, checked: task[1] !== ' ' })); t = t.slice(task[0].length); }
            li.append(inline(t));
            list.append(li);
          }
          out.append(list);
        } else if (/^\|/.test(l) && i + 1 < lines.length && /^\|?\s*:?-{2,}/.test(lines[i + 1])) {
          const cells = (s) => s.replace(/^\||\|\s*$/g, '').split('|').map((c) => c.trim());
          const head = cells(l); i += 2;
          const body = [];
          while (i < lines.length && /^\|/.test(lines[i])) body.push(cells(lines[i++]));
          out.append(h('table', {}, h('thead', {}, h('tr', {}, head.map((c) => h('th', {}, inline(c))))), h('tbody', {}, body.map((r) => h('tr', {}, r.map((c) => h('td', {}, inline(c))))))));
        } else {
          const buf = [l]; i++;
          while (i < lines.length && lines[i].trim() && !isBlockStart(lines[i])) buf.push(lines[i++]);
          out.append(h('p', {}, inline(buf.join(' '))));
        }
      }
      return out;
    }
    function inline(t) {
      const frag = document.createDocumentFragment();
      const re = /(`+)(.+?)\1|\*\*(.+?)\*\*|__(.+?)__|(?<![\w*])\*(?!\s)(.+?)\*(?!\w)|(?<!\w)_(?!\s)(.+?)_(?!\w)|~~(.+?)~~|(!?)\[([^\]]*)\]\(([^)\s]+)(?:\s+"[^"]*")?\)|(https?:\/\/[^\s<)]+)/g;
      let last = 0, m;
      while ((m = re.exec(t))) {
        if (m.index > last) frag.append(t.slice(last, m.index));
        if (m[2] != null) frag.append(h('code', {}, m[2]));
        else if (m[3] != null || m[4] != null) frag.append(h('strong', {}, inline(m[3] ?? m[4])));
        else if (m[5] != null || m[6] != null) frag.append(h('em', {}, inline(m[5] ?? m[6])));
        else if (m[7] != null) frag.append(h('del', {}, inline(m[7])));
        else if (m[10] != null) {
          const href = m[10];
          if (m[8]) frag.append(h('span', { class: 'md-img' }, '🖼 ', m[9] || href));
          else if (/^https?:\/\//i.test(href)) frag.append(h('a', { href, target: '_blank', rel: 'noopener noreferrer' }, inline(m[9] || href)));
          else frag.append(inline(m[9] || href));
        } else if (m[11]) frag.append(h('a', { href: m[11], target: '_blank', rel: 'noopener noreferrer' }, m[11]));
        last = m.index + m[0].length;
      }
      if (last < t.length) frag.append(t.slice(last));
      return frag;
    }

    /* ---------- 3D: STL / OBJ meshes ---------- */
    function meshView(ctx) {
      const { f, url, stage } = ctx;
      const el = h('div', { class: 'v3d' }, h('div', { class: 'vspin' }));
      stage.append(el);
      let alive = true, dispose = null;
      ctx.setCleanup(() => { alive = false; if (dispose) dispose(); });
      fetch(url).then((r) => { if (!r.ok) throw new Error(); return r.arrayBuffer(); }).then((buf) => {
        if (!alive) return;
        const pos = ext(f.name) === 'obj' ? parseOBJ(new TextDecoder().decode(buf)) : parseSTL(buf);
        if (!pos.length) throw new Error();
        el.replaceChildren();
        dispose = scene(el, { mesh: pos });
      }).catch(() => alive && ctx.fail());
    }
    function parseSTL(buf) {
      const dv = new DataView(buf);
      if (buf.byteLength >= 84) {
        const n = dv.getUint32(80, true);
        if (84 + n * 50 === buf.byteLength) {
          const p = new Float32Array(n * 9);
          for (let i = 0; i < n; i++) for (let j = 0; j < 9; j++) p[i * 9 + j] = dv.getFloat32(84 + i * 50 + 12 + j * 4, true);
          return p;
        }
      }
      const txt = new TextDecoder().decode(buf);
      const out = [];
      const re = /vertex\s+([-+\d.eE]+)\s+([-+\d.eE]+)\s+([-+\d.eE]+)/g;
      let m;
      while ((m = re.exec(txt))) out.push(+m[1], +m[2], +m[3]);
      return new Float32Array(out.slice(0, out.length - (out.length % 9)));
    }
    function parseOBJ(txt) {
      const v = [], out = [];
      for (const line of txt.split('\n')) {
        if (line.startsWith('v ')) { const p = line.trim().split(/\s+/); v.push([+p[1], +p[2], +p[3]]); }
        else if (line.startsWith('f ')) {
          const ids = line.trim().split(/\s+/).slice(1).map((s) => { const k = parseInt(s, 10); return k < 0 ? v.length + k : k - 1; });
          for (let i = 1; i + 1 < ids.length; i++) for (const k of [ids[0], ids[i], ids[i + 1]]) { const p = v[k] || [0, 0, 0]; out.push(p[0], p[1], p[2]); }
        }
      }
      return new Float32Array(out);
    }

    /* ---------- G-code toolpath ---------- */
    function gcodeView(el, txt) {
      let x = 0, y = 0, z = 0, abs = true, mode = 0, plane = 17, inch = false;
      const cut = [], rapid = [];
      let moves = 0;
      const push = (arr, a, b, c, d, e, f2) => arr.push(a, b, c, d, e, f2);
      for (const raw of txt.split('\n')) {
        const line = raw.replace(/\(.*?\)|;.*/g, '').toUpperCase();
        if (!line.trim()) continue;
        const w = {};
        for (const m of line.matchAll(/([A-Z])\s*([-+]?\d*\.?\d+)/g)) {
          const L = m[1], val = parseFloat(m[2]);
          if (L === 'G') {
            if (val === 0 || val === 1 || val === 2 || val === 3) mode = val;
            else if (val === 90) abs = true; else if (val === 91) abs = false;
            else if (val === 20) inch = true; else if (val === 21) inch = false;
            else if (val === 17 || val === 18 || val === 19) plane = val;
          } else w[L] = val;
        }
        if (!('X' in w || 'Y' in w || 'Z' in w)) continue;
        const nx = 'X' in w ? (abs ? w.X : x + w.X) : x;
        const ny = 'Y' in w ? (abs ? w.Y : y + w.Y) : y;
        const nz = 'Z' in w ? (abs ? w.Z : z + w.Z) : z;
        moves++;
        if ((mode === 2 || mode === 3) && plane === 17) {
          let cx, cy;
          if ('I' in w || 'J' in w) { cx = x + (w.I || 0); cy = y + (w.J || 0); }
          else if ('R' in w) {
            const dx = nx - x, dy = ny - y, d = Math.hypot(dx, dy), r = Math.abs(w.R);
            const hh = Math.sqrt(Math.max(0, r * r - (d * d) / 4)) * ((mode === 2) === (w.R > 0) ? -1 : 1);
            cx = x + dx / 2 - (hh * dy) / d; cy = y + dy / 2 + (hh * dx) / d;
          } else { push(cut, x, y, z, nx, ny, nz); x = nx; y = ny; z = nz; continue; }
          const r = Math.hypot(x - cx, y - cy);
          let a0 = Math.atan2(y - cy, x - cx), a1 = Math.atan2(ny - cy, nx - cx);
          if (mode === 2) { if (a1 >= a0) a1 -= 2 * Math.PI; } else if (a1 <= a0) a1 += 2 * Math.PI;
          const n = Math.max(4, Math.min(256, Math.ceil(Math.abs(a1 - a0) * Math.max(r, 1) * 2)));
          let px = x, py = y, pz = z;
          for (let i = 1; i <= n; i++) {
            const a = a0 + ((a1 - a0) * i) / n, qz = z + ((nz - z) * i) / n;
            const qx = cx + r * Math.cos(a), qy = cy + r * Math.sin(a);
            push(cut, px, py, pz, qx, qy, qz); px = qx; py = qy; pz = qz;
          }
        } else push(mode === 0 ? rapid : cut, x, y, z, nx, ny, nz);
        x = nx; y = ny; z = nz;
      }
      if (!cut.length && !rapid.length) { el.append(h('div', { class: 'vnone-sub' }, 'No moves found in this file.')); return null; }
      let zmin = Infinity, zmax = -Infinity;
      for (let i = 2; i < cut.length; i += 3) { zmin = Math.min(zmin, cut[i]); zmax = Math.max(zmax, cut[i]); }
      if (zmin < 0) zmax = Math.min(zmax, 0); // Z0 = stock top: colour only below it
      // Colour cuts by depth: shallow = cyan, deep = magenta.
      const col = (zz) => { const t = zmax > zmin ? Math.min(1, Math.max(0, (zmax - zz) / (zmax - zmin))) : 0; return [0.22 + 0.73 * t, 0.78 - 0.45 * t, 0.97 - 0.15 * t]; };
      const lp = new Float32Array(cut.length + rapid.length), lc = new Float32Array(cut.length + rapid.length);
      lp.set(cut); lp.set(rapid, cut.length);
      for (let i = 0; i < cut.length; i += 3) lc.set(col(cut[i + 2]), i);
      for (let i = cut.length; i < lc.length; i += 3) lc.set([0.42, 0.44, 0.5], i);
      const u = inch ? 'in' : 'mm';
      return scene(el, { lines: lp, lineColors: lc, pitch: 60, info: `${nf(moves)} moves · ${u}` });
    }

    /* ---------- minimal WebGL orbit scene ---------- */
    function scene(el, { mesh, lines, lineColors, pitch = 28, info }) {
      const canvas = h('canvas');
      const hud = h('div', { class: 'v3d-hud' });
      const help = h('div', { class: 'v3d-help' }, 'Drag to rotate · Shift-drag to pan · Scroll to zoom · Double-click to reset');
      el.append(canvas, hud, help);
      const gl = canvas.getContext('webgl', { antialias: true, alpha: true }) || canvas.getContext('experimental-webgl');
      if (!gl) { el.replaceChildren(h('div', { class: 'vnone-sub' }, 'Your browser can’t show 3D previews.')); return null; }

      // Bounds
      const all = [mesh, lines].filter(Boolean);
      const mn = [Infinity, Infinity, Infinity], mx = [-Infinity, -Infinity, -Infinity];
      for (const a of all) for (let i = 0; i < a.length; i += 3) for (let k = 0; k < 3; k++) { const q = a[i + k]; if (q < mn[k]) mn[k] = q; if (q > mx[k]) mx[k] = q; }
      const ctr = [0, 1, 2].map((k) => (mn[k] + mx[k]) / 2);
      const size = [0, 1, 2].map((k) => mx[k] - mn[k]);
      const rad = Math.max(1e-3, Math.hypot(...size) / 2);
      const fmt = (n) => (n >= 100 ? n.toFixed(0) : n >= 10 ? n.toFixed(1) : n.toFixed(2));
      hud.textContent = (mesh ? `${nf(mesh.length / 9)} triangles · ` : info ? info + ' · ' : '') + `${fmt(size[0])} × ${fmt(size[1])} × ${fmt(size[2])}`;

      // Floor grid under the part.
      const step = Math.pow(10, Math.floor(Math.log10(Math.max(size[0], size[1], 1e-3)))) / 2 || 1;
      const g = [], gx0 = Math.floor((mn[0] - rad * 0.3) / step) * step, gx1 = Math.ceil((mx[0] + rad * 0.3) / step) * step;
      const gy0 = Math.floor((mn[1] - rad * 0.3) / step) * step, gy1 = Math.ceil((mx[1] + rad * 0.3) / step) * step;
      const gz = mesh ? mn[2] : Math.min(mn[2], 0);
      for (let X = gx0; X <= gx1 + 1e-9 && g.length < 6000; X += step) g.push(X, gy0, gz, X, gy1, gz);
      for (let Y = gy0; Y <= gy1 + 1e-9 && g.length < 12000; Y += step) g.push(gx0, Y, gz, gx1, Y, gz);
      const grid = new Float32Array(g);

      const prog = (vs, fs) => {
        const p = gl.createProgram();
        for (const [t, s] of [[gl.VERTEX_SHADER, vs], [gl.FRAGMENT_SHADER, fs]]) { const sh = gl.createShader(t); gl.shaderSource(sh, s); gl.compileShader(sh); gl.attachShader(p, sh); }
        gl.linkProgram(p); return p;
      };
      const meshP = prog(
        'attribute vec3 p;attribute vec3 n;uniform mat4 m;varying vec3 v;void main(){v=n;gl_Position=m*vec4(p,1.);}',
        'precision mediump float;varying vec3 v;uniform vec3 L;uniform vec3 c;void main(){vec3 N=normalize(v);float d=abs(dot(N,L));float t=max(dot(N,vec3(0.,0.,1.)),0.);vec3 col=c*(.26+.62*d+.16*t)+vec3(.18)*pow(d,24.);gl_FragColor=vec4(col,1.);}');
      const lineP = prog(
        'attribute vec3 p;attribute vec3 c;uniform mat4 m;varying vec3 v;void main(){v=c;gl_Position=m*vec4(p,1.);}',
        'precision mediump float;varying vec3 v;uniform float a;void main(){gl_FragColor=vec4(v,a);}');
      const buf = (data) => { const b = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, b); gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW); return b; };
      let meshB, normB, lineB, lineCB, gridB, gridCB;
      if (mesh) {
        const nrm = new Float32Array(mesh.length);
        for (let i = 0; i < mesh.length; i += 9) {
          const ax = mesh[i + 3] - mesh[i], ay = mesh[i + 4] - mesh[i + 1], az = mesh[i + 5] - mesh[i + 2];
          const bx = mesh[i + 6] - mesh[i], by = mesh[i + 7] - mesh[i + 1], bz = mesh[i + 8] - mesh[i + 2];
          let nx = ay * bz - az * by, ny = az * bx - ax * bz, nz = ax * by - ay * bx;
          const l = Math.hypot(nx, ny, nz) || 1; nx /= l; ny /= l; nz /= l;
          for (let k = 0; k < 9; k += 3) { nrm[i + k] = nx; nrm[i + k + 1] = ny; nrm[i + k + 2] = nz; }
        }
        meshB = buf(mesh); normB = buf(nrm);
      }
      if (lines) { lineB = buf(lines); lineCB = buf(lineColors); }
      gridB = buf(grid); gridCB = buf(new Float32Array(grid.length).fill(0.3));

      const home = { yaw: -Math.PI / 3.2, pitch: (pitch * Math.PI) / 180, dist: rad * 3.5, t: ctr.slice() };
      let cam = { ...home, t: home.t.slice() };
      const sub3 = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
      const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
      const norm = (a) => { const l = Math.hypot(...a) || 1; return [a[0] / l, a[1] / l, a[2] / l]; };
      const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
      function basis() {
        const cp = Math.cos(cam.pitch);
        const eye = [cam.t[0] + cam.dist * cp * Math.cos(cam.yaw), cam.t[1] + cam.dist * cp * Math.sin(cam.yaw), cam.t[2] + cam.dist * Math.sin(cam.pitch)];
        const zz = norm(sub3(eye, cam.t));
        let xx = cross([0, 0, 1], zz);
        xx = Math.hypot(...xx) < 1e-6 ? [1, 0, 0] : norm(xx);
        return { eye, xx, yy: cross(zz, xx), zz };
      }
      let raf = 0;
      const draw = () => { if (!raf) raf = requestAnimationFrame(render); };
      function render() {
        raf = 0;
        const dpr = Math.min(2, devicePixelRatio || 1);
        const w = Math.max(1, Math.round(canvas.clientWidth * dpr)), hgt = Math.max(1, Math.round(canvas.clientHeight * dpr));
        if (canvas.width !== w || canvas.height !== hgt) { canvas.width = w; canvas.height = hgt; }
        gl.viewport(0, 0, w, hgt);
        gl.clearColor(0, 0, 0, 0);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
        gl.enable(gl.DEPTH_TEST);
        const { eye, xx, yy, zz } = basis();
        const near = cam.dist / 200, far = cam.dist + rad * 8;
        const f = 1 / Math.tan((35 * Math.PI) / 360), asp = w / hgt;
        const P = [f / asp, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) / (near - far), -1, 0, 0, (2 * far * near) / (near - far), 0];
        const V = [xx[0], yy[0], zz[0], 0, xx[1], yy[1], zz[1], 0, xx[2], yy[2], zz[2], 0, -dot(xx, eye), -dot(yy, eye), -dot(zz, eye), 1];
        const M = new Float32Array(16);
        for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) { let s = 0; for (let k = 0; k < 4; k++) s += P[k * 4 + r] * V[c * 4 + k]; M[c * 4 + r] = s; }
        const bind = (p, name, b, n) => { const loc = gl.getAttribLocation(p, name); if (loc < 0) return; gl.bindBuffer(gl.ARRAY_BUFFER, b); gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, n, gl.FLOAT, false, 0, 0); };
        // grid
        gl.useProgram(lineP);
        gl.enable(gl.BLEND); gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
        gl.uniformMatrix4fv(gl.getUniformLocation(lineP, 'm'), false, M);
        gl.uniform1f(gl.getUniformLocation(lineP, 'a'), 0.5);
        bind(lineP, 'p', gridB, 3); bind(lineP, 'c', gridCB, 3);
        gl.drawArrays(gl.LINES, 0, grid.length / 3);
        if (lines) {
          gl.uniform1f(gl.getUniformLocation(lineP, 'a'), 1);
          bind(lineP, 'p', lineB, 3); bind(lineP, 'c', lineCB, 3);
          gl.drawArrays(gl.LINES, 0, lines.length / 3);
        }
        gl.disable(gl.BLEND);
        if (mesh) {
          gl.useProgram(meshP);
          gl.uniformMatrix4fv(gl.getUniformLocation(meshP, 'm'), false, M);
          const L = norm([zz[0] + xx[0] * 0.35 + yy[0] * 0.5, zz[1] + xx[1] * 0.35 + yy[1] * 0.5, zz[2] + xx[2] * 0.35 + yy[2] * 0.5]);
          gl.uniform3fv(gl.getUniformLocation(meshP, 'L'), L);
          gl.uniform3fv(gl.getUniformLocation(meshP, 'c'), [0.66, 0.72, 0.82]);
          bind(meshP, 'p', meshB, 3); bind(meshP, 'n', normB, 3);
          gl.drawArrays(gl.TRIANGLES, 0, mesh.length / 3);
        }
      }

      let drag = null;
      canvas.addEventListener('pointerdown', (e) => { canvas.setPointerCapture(e.pointerId); drag = { x: e.clientX, y: e.clientY, pan: e.shiftKey || e.button === 2 }; help.classList.add('gone'); });
      canvas.addEventListener('pointermove', (e) => {
        if (!drag) return;
        const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
        drag.x = e.clientX; drag.y = e.clientY;
        if (drag.pan) {
          const { xx, yy } = basis(), k = (cam.dist * 0.0018);
          for (let i = 0; i < 3; i++) cam.t[i] += (-dx * xx[i] + dy * yy[i]) * k;
        } else {
          cam.yaw -= dx * 0.008;
          cam.pitch = Math.max(-1.55, Math.min(1.55, cam.pitch + dy * 0.008));
        }
        draw();
      });
      const end = () => { drag = null; };
      canvas.addEventListener('pointerup', end);
      canvas.addEventListener('pointercancel', end);
      canvas.addEventListener('contextmenu', (e) => e.preventDefault());
      canvas.addEventListener('wheel', (e) => { e.preventDefault(); cam.dist = Math.max(rad * 0.05, Math.min(rad * 20, cam.dist * Math.exp(e.deltaY * 0.0015))); draw(); }, { passive: false });
      canvas.addEventListener('dblclick', () => { cam = { ...home, t: home.t.slice() }; draw(); });
      const ro = new ResizeObserver(draw);
      ro.observe(el);
      draw();
      return () => {
        ro.disconnect(); cancelAnimationFrame(raf);
        const ext = gl.getExtension('WEBGL_lose_context'); if (ext) ext.loseContext();
      };
    }

    addEventListener('keydown', key, true);
    document.documentElement.classList.add('viewer-open');
    document.body.append(v);
    show();
    return { close };
  }

  window.VXViewer = { open };
})();
