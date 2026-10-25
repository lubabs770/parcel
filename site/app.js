// parcel site: theme switcher, copy buttons, docs scroll-spy. No dependencies.
const root = document.documentElement;
const picks = [...document.querySelectorAll('[data-pick]')];
const names = ['tokyo-night', 'catppuccin', 'gruvbox', 'rose-pine', 'nord', 'everforest', 'kanagawa', 'latte'];

// Every palette lives in style.css as [data-t=name] custom properties; switching is one attribute.
function setTheme(name, save) {
  root.dataset.t = name;
  picks.forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.pick === name)));
  const label = document.getElementById('tn');
  if (label) label.textContent = name;
  if (save) try { localStorage.setItem('parcel-theme', name); } catch (e) {}
}

let saved = null;
try { saved = localStorage.getItem('parcel-theme'); } catch (e) {}
if (names.includes(saved)) setTheme(saved);
else if (matchMedia('(prefers-color-scheme: light)').matches) setTheme('latte');

picks.forEach((b) => b.addEventListener('click', () => setTheme(b.dataset.pick, true)));

// `t` cycles themes, as in the TUI (landing page only, never while typing).
document.addEventListener('keydown', (e) => {
  if (e.key !== 't' || !picks.length || e.metaKey || e.ctrlKey || e.altKey || /INPUT|TEXTAREA/.test(e.target.tagName)) return;
  const i = names.indexOf(root.dataset.t || 'tokyo-night');
  setTheme(names[(i + 1) % names.length], true);
});

// Copy buttons copy the <code> that sits next to them.
document.querySelectorAll('.copy').forEach((b) => b.addEventListener('click', async () => {
  const text = b.parentElement.querySelector('code').innerText.trim();
  try {
    await navigator.clipboard.writeText(text);
    b.textContent = 'copied';
    b.classList.add('done');
  } catch (e) {
    b.textContent = 'copy failed';
  }
  setTimeout(() => { b.textContent = 'copy'; b.classList.remove('done'); }, 1600);
}));

// Docs: highlight the TOC entry for the section in view.
const toc = document.querySelectorAll('.toc a');
if (toc.length && 'IntersectionObserver' in window) {
  const io = new IntersectionObserver((entries) => {
    entries.forEach((en) => {
      if (!en.isIntersecting) return;
      toc.forEach((a) => {
        const on = a.getAttribute('href') === '#' + en.target.id;
        a.classList.toggle('on', on);
        if (on) a.parentElement.scrollTo({ left: a.offsetLeft - 16 }); // the mobile TOC is a scrolling row
      });
    });
  }, { rootMargin: '-15% 0px -75% 0px' });
  document.querySelectorAll('.doc section[id]').forEach((s) => io.observe(s));
}
