// Formatos de exibição no estilo do CM 01/02.

/** "2007-07-13" -> "13.7.2007" */
export function cmDate(iso: string | null | undefined): string {
  if (!iso) return '-'
  const [y, m, d] = iso.split('-').map(Number)
  return `${d}.${m}.${y}`
}

/** 11000 -> "€11K", 2500000 -> "€2.5M" */
export function money(v: number | null | undefined): string {
  if (!v) return '-'
  const sign = v < 0 ? '-' : ''
  const a = Math.abs(v)
  if (a >= 1_000_000) return `${sign}€${(a / 1_000_000).toFixed(a % 1_000_000 ? 1 : 0)}M`
  if (a >= 1_000) return `${sign}€${Math.round(a / 1_000)}K`
  return `${sign}€${a}`
}

/** "de Arrascaeta, Giorgian" -> "de Arrascaeta, G" (como no plantel do CM); nomes comuns ficam iguais. */
export function shortListName(listName: string): string {
  const i = listName.indexOf(', ')
  return i < 0 ? listName : `${listName.slice(0, i)}, ${listName[i + 2] ?? ''}`
}
