import type { ScoutFind } from './bindings/ScoutFind'
import type { MenuOption } from './components/Screen'

/** O Filtrar da lista de um olheiro: posição, lado e estrelas mínimas (como o do plantel). */
export type ScoutFilter = { line: string; side: string; stars: number }
export const NO_SCOUT_FILTER: ScoutFilter = { line: '', side: '', stars: 0 }

export const SCOUT_FILTER_OPTIONS: MenuOption<string>[] = [
  { key: 'line:', label: 'Todas Posições' },
  { key: 'line:GR', label: 'Guarda-Redes' },
  { key: 'line:D', label: 'Defesas' },
  { key: 'line:M', label: 'Médios' },
  { key: 'line:A', label: 'Atacantes' },
  'separator',
  { key: 'side:', label: 'Todos Lados' },
  { key: 'side:E', label: 'Lado Esquerdo' },
  { key: 'side:C', label: 'Central' },
  { key: 'side:D', label: 'Lado Direito' },
  'separator',
  { key: 'stars:0', label: 'Todas Recomendações' },
  { key: 'stars:3', label: '*** ou mais' },
  { key: 'stars:4', label: '**** ou mais' },
  { key: 'stars:5', label: '*****' },
]

export function scoutPasses(f: ScoutFind, filter: ScoutFilter) {
  return (!filter.line || f.lines.includes(filter.line)) && (!filter.side || f.sides.includes(filter.side)) && f.stars >= filter.stars
}

/** Marca a opção escolhida no menu. */
export function scoutChecked(filter: ScoutFilter, key: string) {
  const [group, value = ''] = key.split(':')
  return group === 'line' ? filter.line === value : group === 'side' ? filter.side === value : filter.stars === Number(value)
}

export function scoutChange(filter: ScoutFilter, key: string): ScoutFilter {
  const [group, value = ''] = key.split(':')
  return group === 'line' ? { ...filter, line: value } : group === 'side' ? { ...filter, side: value } : { ...filter, stars: Number(value) }
}
