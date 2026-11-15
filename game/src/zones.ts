import type { TableZone } from './bindings/TableZone'

// As faixas da classificação (vagas, subida, descida), como as linhas do CM: a Classificação
// da competição e a da partida ao vivo.

/** As cores das faixas. */
export const ZONE_COLOURS: Record<string, string> = {
  continental: '#1f9d3a',
  continental2: '#2f6fd6',
  continental3: '#16a3a3',
  promotion: '#1f9d3a',
  relegation: '#c02020',
}

/** A faixa do lugar `pos` (1 = primeiro) numa tabela de `n` clubes. */
export function zoneAt(zones: TableZone[], pos: number, n: number) {
  return zones.find((z) => (z.fromBottom ? pos >= n - z.to + 1 && pos <= n - z.from + 1 : pos >= z.from && pos <= z.to))
}

/** O fundo da linha inteira na cor da faixa (transparente). */
export function zoneRowStyle(zones: TableZone[], pos: number, n: number) {
  const colour = ZONE_COLOURS[zoneAt(zones, pos, n)?.kind ?? '']
  return colour ? { background: `${colour}30` } : undefined
}
