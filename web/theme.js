// Theme: "auto" (follow the OS), "light" or "dark". Loaded synchronously in <head> so the
// page never flashes the wrong theme. Stored per browser in localStorage.
(() => {
  const KEY = 'vx-theme';
  const mq = matchMedia('(prefers-color-scheme: dark)');
  const SUN = '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>';
  const MOON = '<path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/>';

  const pref = () => { try { return localStorage.getItem(KEY) || 'auto'; } catch { return 'auto'; } };
  const effective = () => { const p = pref(); return p === 'auto' ? (mq.matches ? 'dark' : 'light') : p; };

  function apply() {
    const p = pref(), eff = effective(), d = document.documentElement;
    if (p === 'auto') d.removeAttribute('data-theme'); else d.setAttribute('data-theme', p);
    d.style.colorScheme = eff;
    document.querySelectorAll('meta[name="theme-color"]').forEach((m) => { m.removeAttribute('media'); m.content = eff === 'dark' ? '#0d0e12' : '#f7f7f9'; });
    document.querySelectorAll('.theme-btn').forEach(paint);
    window.dispatchEvent(new CustomEvent('vx-theme', { detail: p }));
  }
  function paint(b) {
    const dark = effective() === 'dark';
    b.innerHTML = `<svg class="i" viewBox="0 0 24 24">${dark ? SUN : MOON}</svg>`;
    b.title = dark ? 'Switch to light mode' : 'Switch to dark mode';
    b.setAttribute('aria-label', b.title);
  }
  function set(p) {
    try { if (p === 'auto') localStorage.removeItem(KEY); else localStorage.setItem(KEY, p); } catch {}
    const d = document.documentElement;
    d.classList.add('theme-anim');
    apply();
    clearTimeout(set.t); set.t = setTimeout(() => d.classList.remove('theme-anim'), 300);
  }
  function button(cls = 'btn ghost icon') {
    const b = document.createElement('button');
    b.type = 'button';
    b.className = cls + ' theme-btn';
    b.addEventListener('click', (e) => { e.stopPropagation(); set(effective() === 'dark' ? 'light' : 'dark'); });
    paint(b);
    return b;
  }

  window.VXTheme = { pref, effective, set, button };
  mq.addEventListener('change', apply);
  addEventListener('storage', (e) => { if (e.key === KEY) apply(); });
  apply();
})();
