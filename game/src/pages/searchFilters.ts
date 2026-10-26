import type { PersonKind } from '../bindings/PersonKind'
import type { Position } from '../bindings/Position'
import type { SearchQuery } from '../bindings/SearchQuery'
import type { Side } from '../bindings/Side'

// Opções do diálogo de filtros, nos mesmos grupos do CM 01/02.

export const AGE_BANDS = [
  { key: '0-18', label: '0-18', max: 18 },
  { key: '19-25', label: '19-25', min: 19, max: 25 },
  { key: '26-30', label: '26-30', min: 26, max: 30 },
  { key: '31-35', label: '31-35', min: 31, max: 35 },
  { key: '36+', label: '36+', min: 36 },
] as const

export const POSITION_GROUPS: { key: string; label: string; positions: Position[] }[] = [
  { key: 'GR', label: 'GR', positions: ['GK'] },
  { key: 'Def', label: 'Def', positions: ['D', 'WB'] },
  { key: 'Med', label: 'Méd', positions: ['DM', 'M', 'AM'] },
  { key: 'Ava', label: 'Ava', positions: ['F'] },
]

export const SIDES: { key: Side; label: string }[] = [
  { key: 'left', label: 'Esquerda' },
  { key: 'center', label: 'Centro' },
  { key: 'right', label: 'Direita' },
]

export type Filters = {
  text: string
  nationId: number | null
  age: string | null
  positionGroup: string | null
  side: Side | null
  /** Só os com o contrato a acabar. */
  expiring: boolean
  /** Lesões: só os lesionados ou sem eles. */
  injured: 'only' | 'exclude' | null
  /** Só os com propostas ou na Lista Preferencial. */
  interested: boolean
  /** "Incluir Plantel": com os do clube do treinador. */
  ownSquad: boolean
}

export const EMPTY_FILTERS: Filters = {
  text: '',
  nationId: null,
  age: null,
  positionGroup: null,
  side: null,
  expiring: false,
  injured: null,
  interested: false,
  ownSquad: true,
}

/** `own`: o clube do treinador (para o "Incluir Plantel"). */
export function toQuery(f: Filters, kind: PersonKind, own?: number): SearchQuery {
  const age = AGE_BANDS.find((b) => b.key === f.age)
  const players = kind === 'players'
  return {
    text: f.text.trim() || undefined,
    kind,
    nationId: f.nationId ?? undefined,
    minAge: age && 'min' in age ? age.min : undefined,
    maxAge: age && 'max' in age ? age.max : undefined,
    positions: players ? POSITION_GROUPS.find((g) => g.key === f.positionGroup)?.positions : undefined,
    side: players ? (f.side ?? undefined) : undefined,
    expiring: players && f.expiring ? true : undefined,
    injured: players && f.injured ? f.injured === 'only' : undefined,
    interested: players && f.interested ? true : undefined,
    excludeClub: !f.ownSquad && own !== undefined ? own : undefined,
  }
}

/** Resumo dos filtros ativos, mostrado acima da lista (como "Famous players in your region"). */
export function describe(f: Filters, nationName: (id: number) => string | undefined): string {
  const parts: string[] = []
  if (f.text.trim()) parts.push(`"${f.text.trim()}"`)
  if (f.nationId !== null) parts.push(nationName(f.nationId) ?? '')
  if (f.age) parts.push(`${f.age} anos`)
  if (f.positionGroup) parts.push(POSITION_GROUPS.find((g) => g.key === f.positionGroup)!.label)
  if (f.side) parts.push(SIDES.find((s) => s.key === f.side)!.label)
  if (f.expiring) parts.push('contrato a acabar')
  if (f.injured) parts.push(f.injured === 'only' ? 'lesionados' : 'sem lesionados')
  if (f.interested) parts.push('interessados')
  if (!f.ownSquad) parts.push('sem o plantel')
  return parts.length ? parts.join(' · ') : 'Todos'
}
