const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches

// Download: o setup.exe da última versão (pre-releases incluídas).
// Sem a API, os botões continuam a ir para a página das versões.
;(async () => {
  const metas = document.querySelectorAll('.download-meta')
  const fallback = () => metas.forEach((m) => (m.textContent = 'Windows 10 e 11 · grátis'))
  try {
    const res = await fetch('https://api.github.com/repos/Victorb999/NextCm/releases?per_page=10')
    if (!res.ok) return fallback()
    for (const rel of await res.json()) {
      const asset = !rel.draft && rel.assets.find((a) => a.name.endsWith('-setup.exe'))
      if (!asset) continue
      // v0.1.0-beta.1 → 0.1.0 beta 1
      const version = rel.tag_name.replace(/^v/, '').replace(/-beta\.(\d+)$/, ' beta $1')
      const mb = Math.round(asset.size / 1048576)
      document.querySelectorAll('a.download').forEach((a) => (a.href = asset.browser_download_url))
      metas.forEach((m) => (m.textContent = `Versão ${version} · ${mb} MB · Windows 10 e 11`))
      document.getElementById('pill-version').textContent = `Versão ${version}`
      return
    }
    fallback()
  } catch {
    fallback()
  }
})()

// Menu do telemóvel
const toggle = document.getElementById('nav-toggle')
const menu = document.getElementById('menu')
function setMenu(open) {
  toggle.setAttribute('aria-expanded', String(open))
  toggle.setAttribute('aria-label', open ? 'Fechar menu' : 'Abrir menu')
  document.body.style.overflow = open ? 'hidden' : ''
  if (open) {
    menu.hidden = false
    const items = [...menu.children]
    const step = Math.min(60, 400 / items.length)
    items.forEach((el, i) => (el.style.transitionDelay = `${i * step}ms`))
    requestAnimationFrame(() => requestAnimationFrame(() => menu.classList.add('is-open')))
  } else {
    menu.classList.remove('is-open')
    menu.hidden = true
  }
}
toggle.addEventListener('click', () => setMenu(toggle.getAttribute('aria-expanded') !== 'true'))
menu.addEventListener('click', (e) => e.target.closest('a') && setMenu(false))
addEventListener('keydown', (e) => e.key === 'Escape' && setMenu(false))

// Secções a subir ao entrar no ecrã (uma vez só)
const reveal = new IntersectionObserver(
  (entries) => {
    for (const e of entries) {
      if (!e.isIntersecting) continue
      e.target.classList.add('in')
      reveal.unobserve(e.target)
    }
  },
  { rootMargin: '0px 0px -10% 0px' },
)
document.querySelectorAll('.reveal').forEach((el, i) => {
  // Cartões lado a lado entram em cascata
  if (el.classList.contains('card') || el.classList.contains('step')) el.style.transitionDelay = `${(i % 3) * 80}ms`
  reveal.observe(el)
})

// Print do topo: inclinado, endireita com o scroll
const shot = document.getElementById('hero-shot')
if (!reduced) {
  let ticking = false
  const tilt = () => {
    const p = Math.min(1, Math.max(0, scrollY / (innerHeight * 0.6)))
    shot.style.setProperty('--tilt', `${16 * (1 - p)}deg`)
    shot.style.setProperty('--zoom', String(0.94 + 0.06 * p))
    ticking = false
  }
  addEventListener('scroll', () => {
    if (!ticking) {
      ticking = true
      requestAnimationFrame(tilt)
    }
  }, { passive: true })
  tilt()
}

// Frase: cada palavra acende ao cruzar a linha do meio do ecrã
const tagline = document.getElementById('tagline')
tagline.innerHTML = tagline.textContent
  .trim()
  .split(/\s+/)
  .map((w) => `<span class="w">${w}</span>`)
  .join(' ')
if (!reduced) {
  const words = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        if (!e.isIntersecting) continue
        e.target.classList.add('on')
        words.unobserve(e.target)
      }
    },
    { rootMargin: '0px 0px -45% 0px' },
  )
  tagline.querySelectorAll('.w').forEach((w) => words.observe(w))
} else {
  tagline.querySelectorAll('.w').forEach((w) => w.classList.add('on'))
}

// Números que contam ao aparecer
const counters = new IntersectionObserver((entries) => {
  for (const e of entries) {
    if (!e.isIntersecting) continue
    counters.unobserve(e.target)
    const end = Number(e.target.dataset.count)
    if (reduced || end === 0) continue
    const t0 = performance.now()
    const run = (t) => {
      const k = Math.min(1, (t - t0) / 1200)
      const eased = 1 - Math.pow(1 - k, 4)
      e.target.textContent = String(Math.round(end * eased))
      if (k < 1) requestAnimationFrame(run)
    }
    requestAnimationFrame(run)
  }
})
document.querySelectorAll('[data-count]').forEach((el) => counters.observe(el))

// Tour: troca de tela sozinha a cada 5 s; um clique escolhe e reinicia a contagem
const tabs = [...document.querySelectorAll('.tab')]
const [imgA, imgB] = document.querySelectorAll('.tour-img')
const bar = document.getElementById('tour-progress')
let front = imgA
let current = 0
let timer = 0
const TOUR_MS = 5000
tabs.forEach((t) => {
  const pre = new Image()
  pre.src = t.dataset.img
})
function show(i) {
  current = i
  tabs.forEach((t, k) => t.setAttribute('aria-selected', String(k === i)))
  const back = front === imgA ? imgB : imgA
  back.src = tabs[i].dataset.img
  back.alt = tabs[i].dataset.alt
  back.removeAttribute('aria-hidden')
  front.setAttribute('aria-hidden', 'true')
  front.alt = ''
  back.classList.add('is-on')
  front.classList.remove('is-on')
  front = back
  // A aba escolhida fica à vista na fila (no telemóvel a fila desliza)
  tabs[i].parentElement.scrollTo({ left: tabs[i].offsetLeft - 16, behavior: reduced ? 'auto' : 'smooth' })
  restart()
}
function restart() {
  clearTimeout(timer)
  if (reduced) return
  bar.style.transition = 'none'
  bar.style.transform = 'scaleX(0)'
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      bar.style.transition = `transform ${TOUR_MS}ms cubic-bezier(0.22, 1, 0.36, 1)`
      bar.style.transform = 'scaleX(1)'
    }),
  )
  timer = setTimeout(() => show((current + 1) % tabs.length), TOUR_MS)
}
tabs.forEach((t, i) => t.addEventListener('click', () => show(i)))
// Só começa a rodar quando o tour aparece no ecrã
const tourWatch = new IntersectionObserver((entries) => {
  if (entries[0].isIntersecting) {
    restart()
    tourWatch.disconnect()
  }
})
tourWatch.observe(document.getElementById('tour'))

// Link da secção atual na barra
const links = [...document.querySelectorAll('.nav-links a')]
const spy = new IntersectionObserver(
  (entries) => {
    for (const e of entries) {
      if (!e.isIntersecting) continue
      links.forEach((a) => a.setAttribute('aria-current', String(a.hash === '#' + e.target.id)))
    }
  },
  { rootMargin: '-40% 0px -55% 0px' },
)
links.forEach((a) => {
  const s = document.querySelector(a.hash)
  if (s) spy.observe(s)
})
