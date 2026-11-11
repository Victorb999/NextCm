// Botão de download: o setup.exe da última versão (pre-releases incluídas).
// Sem a API, o link continua a ir para a página das versões.
(async () => {
  const link = document.getElementById('download')
  const sub = document.getElementById('download-sub')
  try {
    const res = await fetch('https://api.github.com/repos/Victorb999/NextCm/releases?per_page=10')
    if (!res.ok) return
    for (const rel of await res.json()) {
      if (rel.draft) continue
      const asset = rel.assets.find((a) => a.name.endsWith('-setup.exe'))
      if (!asset) continue
      link.href = asset.browser_download_url
      const mb = (asset.size / 1048576).toFixed(0)
      sub.textContent = `${rel.tag_name.replace(/^v/, 'versão ')} · ${mb} MB`
      return
    }
  } catch {
    // fica o link para as versões
  }
})()

// Prints em tamanho real
const viewer = document.getElementById('viewer')
const big = viewer.querySelector('img')
for (const btn of document.querySelectorAll('[data-full]')) {
  btn.addEventListener('click', () => {
    big.src = btn.dataset.full
    big.alt = btn.querySelector('img').alt
    viewer.showModal()
  })
}
viewer.addEventListener('click', (e) => {
  if (e.target === viewer) viewer.close()
})
