// File-type icons shared by the app and public share pages.
// window.FileIcons.el(name, isDir, big) -> <svg>; .type(name, isDir) -> { t, color, label }
(() => {
  const T = {
    folder:  { color: '#7468f8' },
    image:   { color: '#e9509b', ext: 'jpg jpeg png gif webp avif bmp ico heic heif svg tif tiff raw cr2 nef dng psd' },
    video:   { color: '#f77a2a', ext: 'mp4 webm mov m4v mkv avi wmv mpg mpeg' },
    audio:   { color: '#12b0a0', ext: 'mp3 wav ogg m4a flac aac opus aiff wma' },
    pdf:     { color: '#ec4545', ext: 'pdf' },
    archive: { color: '#e0a20a', ext: 'zip tar gz tgz bz2 xz 7z rar dmg iso pkg' },
    cnc:     { color: '#0ea5e9', ext: 'gcode nc ngc tap gc cnc' },
    model:   { color: '#20ad5c', ext: 'stl obj 3mf step stp iges igs f3d f3z dxf dwg scad blend fbx gltf glb ply sldprt sldasm' },
    sheet:   { color: '#169c4f', ext: 'xls xlsx ods csv tsv numbers' },
    slides:  { color: '#ef5b2b', ext: 'ppt pptx odp key' },
    doc:     { color: '#2b7cd3', ext: 'doc docx odt rtf pages' },
    code:    { color: '#5b5bd6', ext: 'js mjs ts tsx jsx json rs py go c h cpp hpp cs java kt swift rb php sh zsh bash yml yaml toml xml html css scss sql ini conf cfg env lua dockerfile makefile' },
    text:    { color: '#8a93a3', ext: 'txt md log' },
    font:    { color: '#a35ae8', ext: 'ttf otf woff woff2' },
    other:   { color: '#9aa3b2' },
  };
  const BY_EXT = {};
  for (const [t, v] of Object.entries(T)) if (v.ext) for (const x of v.ext.split(' ')) BY_EXT[x] ??= t;

  const extOf = (n) => { const i = n.lastIndexOf('.'); return i > 0 ? n.slice(i + 1).toLowerCase() : n.toLowerCase(); };
  function type(name, isDir) {
    if (isDir) return { t: 'folder', color: T.folder.color, label: '' };
    const x = extOf(name);
    const t = BY_EXT[x] || 'other';
    const label = x && x !== name.toLowerCase() && x.length <= 4 ? x.toUpperCase() : '';
    return { t, color: T[t].color, label };
  }

  // Glyphs, drawn white in a 48×48 box centred on (24.5, 26).
  const S = 'fill="none" stroke="#fff" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round"';
  const zip = (() => { let s = ''; for (let y = 6; y < 24; y += 3.5) s += `<rect x="${(y / 3.5) % 2 < 1 ? 22.2 : 24.8}" y="${y}" width="2.6" height="2.4" rx=".5" fill="#fff"/>`; return s; })();
  const G = {
    image: '<path d="M16.5 33l5.5-7 4 4.6 2.8-3.2 5.2 5.6z" fill="#fff"/><circle cx="28.8" cy="20.4" r="2.4" fill="#fff"/>',
    video: '<path d="M21 19.5v13l10.5-6.5z" fill="#fff" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/>',
    audio: `<path d="M21.5 32V20.5l9.5-2v11" ${S}/><circle cx="19" cy="32" r="2.7" fill="#fff"/><circle cx="28.5" cy="29.6" r="2.7" fill="#fff"/>`,
    pdf: `<path d="M18 20.5h13M18 25h13M18 29.5h8" ${S}/>`,
    archive: zip + '<rect x="21.5" y="26" width="6" height="7.5" rx="1.6" fill="none" stroke="#fff" stroke-width="2"/>',
    cnc: `<path d="M17 32.5v-11h3.8v11h3.8v-11h3.8v11h3.8v-11" ${S}/>`,
    model: `<path d="M24.5 17.5l7.5 4.2v8.6l-7.5 4.2-7.5-4.2v-8.6z" ${S}/><path d="M17.2 21.8l7.3 4.1 7.3-4.1M24.5 26v8.3" ${S}/>`,
    sheet: `<rect x="17" y="19" width="15" height="14" rx="2" ${S}/><path d="M17 23.7h15M17 28.3h15M22.2 19v14" ${S}/>`,
    slides: `<path d="M19.5 32.5v-5M24.5 32.5v-11M29.5 32.5v-7.5" ${S} stroke-width="2.8"/>`,
    doc: `<path d="M18 20.5h13M18 25h13M18 29.5h9" ${S}/>`,
    code: `<path d="M20.5 21l-4.5 5 4.5 5M28.5 21l4.5 5-4.5 5M26.3 19.2l-3.6 13.6" ${S}/>`,
    text: `<path d="M18 20.5h13M18 25h13M18 29.5h7" ${S}/>`,
    font: '<text x="24.5" y="31" text-anchor="middle" font-family="Georgia, serif" font-size="13" font-weight="700" fill="#fff">Aa</text>',
    other: '<circle cx="19.5" cy="26" r="1.8" fill="#fff"/><circle cx="24.5" cy="26" r="1.8" fill="#fff"/><circle cx="29.5" cy="26" r="1.8" fill="#fff"/>',
  };

  const FOLDER = (c) =>
    `<path d="M4 12.5A4.5 4.5 0 0 1 8.5 8h9.6a4.5 4.5 0 0 1 3.2 1.3l2.4 2.4a4.5 4.5 0 0 0 3.2 1.3h12.6a4.5 4.5 0 0 1 4.5 4.5v19A4.5 4.5 0 0 1 39.5 41h-31A4.5 4.5 0 0 1 4 36.5z" fill="${c}"/>` +
    `<path d="M4 12.5A4.5 4.5 0 0 1 8.5 8h9.6a4.5 4.5 0 0 1 3.2 1.3l2.4 2.4a4.5 4.5 0 0 0 3.2 1.3h12.6a4.5 4.5 0 0 1 4.5 4.5v19A4.5 4.5 0 0 1 39.5 41h-31A4.5 4.5 0 0 1 4 36.5z" fill="#000" opacity=".18"/>` +
    `<path d="M4 21a4 4 0 0 1 4-4h32a4 4 0 0 1 4 4v15.5a4.5 4.5 0 0 1-4.5 4.5h-31A4.5 4.5 0 0 1 4 36.5z" fill="${c}"/>` +
    `<path d="M4 21a4 4 0 0 1 4-4h32a4 4 0 0 1 4 4v3H4z" fill="#fff" opacity=".16"/>` +
    `<path d="M8 17.6h32" stroke="#fff" stroke-opacity=".35" stroke-width="1"/>`;

  const PAGE = 'M13.5 4H30l10.5 10.5V40a4 4 0 0 1-4 4h-23a4 4 0 0 1-4-4V8a4 4 0 0 1 4-4z';
  function file(t, color, label, big) {
    const showLabel = big && label;
    const gy = showLabel ? -3.5 : 0, sc = showLabel ? 0.86 : 1;
    let s = `<path d="${PAGE}" fill="${color}"/>` +
      '<path d="M13.5 4H30l10.5 10.5V22H9.5V8a4 4 0 0 1 4-4z" fill="#fff" opacity=".1"/>' +
      '<path d="M30 4v7a3.5 3.5 0 0 0 3.5 3.5h7z" fill="#fff" opacity=".42"/>' +
      `<g transform="translate(24.5 ${26 + gy}) scale(${sc}) translate(-24.5 -26)">${G[t] || G.other}</g>`;
    if (showLabel) s += `<text x="25" y="39.6" text-anchor="middle" font-family="ui-sans-serif,-apple-system,system-ui,sans-serif" font-size="${label.length > 3 ? 6.2 : 7}" font-weight="800" letter-spacing=".3" fill="#fff" opacity=".95">${label}</text>`;
    return s;
  }

  const cache = new Map();
  function el(name, isDir, big = false) {
    const ty = type(name, isDir);
    const key = ty.t + '|' + (big ? ty.label : '') + '|' + big;
    let inner = cache.get(key);
    if (!inner) { inner = ty.t === 'folder' ? FOLDER(ty.color) : file(ty.t, ty.color, ty.label, big); cache.set(key, inner); }
    const s = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    s.setAttribute('viewBox', '0 0 48 48');
    s.setAttribute('class', 'fti');
    s.setAttribute('aria-hidden', 'true');
    s.innerHTML = inner;
    return s;
  }

  window.FileIcons = { el, type };
})();
