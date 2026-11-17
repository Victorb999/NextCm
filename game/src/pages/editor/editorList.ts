// A lista de resultados do Editor de Jogos Salvos: as colunas de cada modo e a ordenação (só a
// ordem em que se mostram as linhas que o core devolveu).

import type { EditorHit } from '../../bindings/EditorHit'
import { cmDate, money, wage } from '../../format'
import { CURRENCIES, getSettings } from '../../settings'

export type Kind = 'players' | 'staff' | 'clubs'
export type SortKey =
  | 'name'
  | 'nation'
  | 'club'
  | 'position'
  | 'ability'
  | 'potential'
  | 'age'
  | 'value'
  | 'wage'
  | 'expires'
  | 'reputation'
  | 'status'
export type Sort = { key: SortKey; desc: boolean }

export type Column = {
  key: SortKey
  label: string
  /** Classe da largura (as de texto crescem). */
  width: string
  num?: boolean
  show: (h: EditorHit) => string
}

const text = (v: string | null | undefined) => v ?? '-'
const num = (v: number | null | undefined) => (v == null ? '-' : String(v))

const NAME: Column = { key: 'name', label: 'Nome', width: 'min-w-[160px] flex-[2]', show: (h) => h.name }
const NATION: Column = { key: 'nation', label: 'Nação', width: 'w-24', show: (h) => text(h.nation) }
const CLUB: Column = { key: 'club', label: 'Clube', width: 'min-w-[130px] flex-[1.5]', show: (h) => h.club ?? 'Sem clube' }
const CA: Column = { key: 'ability', label: 'CA', width: 'w-9', num: true, show: (h) => num(h.ability) }
const PA: Column = { key: 'potential', label: 'PA', width: 'w-9', num: true, show: (h) => num(h.potential) }
const AGE: Column = { key: 'age', label: 'Idade', width: 'w-11', num: true, show: (h) => num(h.age) }
const WAGE: Column = { key: 'wage', label: 'Salário', width: 'w-24', num: true, show: (h) => wage(h.wage) }
const EXPIRES: Column = { key: 'expires', label: 'Contrato até', width: 'w-24', num: true, show: (h) => cmDate(h.expires) }
const REP: Column = { key: 'reputation', label: 'Rep.', width: 'w-12', num: true, show: (h) => num(h.reputation) }

export const COLUMNS: Record<Kind, Column[]> = {
  players: [
    NAME,
    NATION,
    CLUB,
    { key: 'position', label: 'Posição', width: 'w-24', show: (h) => text(h.position) || '-' },
    CA,
    PA,
    AGE,
    { key: 'value', label: 'Valor', width: 'w-20', num: true, show: (h) => money(h.value) },
    WAGE,
    EXPIRES,
    REP,
    { key: 'status', label: 'Situação', width: 'min-w-[80px] flex-1', show: (h) => h.status || '-' },
  ],
  staff: [
    NAME,
    NATION,
    CLUB,
    { key: 'position', label: 'Cargo', width: 'min-w-[120px] flex-1', show: (h) => text(h.position) },
    CA,
    PA,
    AGE,
    WAGE,
    EXPIRES,
    REP,
  ],
  clubs: [
    NAME,
    NATION,
    { key: 'position', label: 'Divisão', width: 'min-w-[160px] flex-[1.5]', show: (h) => text(h.position) },
    { ...REP, label: 'Reputação', width: 'w-24' },
  ],
}

/** A ordem de partida de cada modo (a do core: os melhores primeiro). */
export const DEFAULT_SORT: Record<Kind, Sort> = {
  players: { key: 'ability', desc: true },
  staff: { key: 'ability', desc: true },
  clubs: { key: 'reputation', desc: true },
}

function valueOf(h: EditorHit, key: SortKey): string | number | null {
  const v = h[key]
  return v === '' ? null : (v ?? null)
}

/** As linhas pela coluna escolhida; os vazios sempre no fim, e o nome desempata. */
export function sortHits(hits: EditorHit[], sort: Sort): EditorHit[] {
  const dir = sort.desc ? -1 : 1
  return [...hits].sort((a, b) => {
    const x = valueOf(a, sort.key)
    const y = valueOf(b, sort.key)
    if (x === null || y === null) return x === y ? a.name.localeCompare(b.name, 'pt') : x === null ? 1 : -1
    const c = typeof x === 'number' && typeof y === 'number' ? x - y : String(x).localeCompare(String(y), 'pt')
    return c * dir || a.name.localeCompare(b.name, 'pt')
  })
}

/** Clicar no cabeçalho: a mesma coluna inverte; outra começa pelos maiores (números) ou de A a Z. */
export function nextSort(cur: Sort, col: Column): Sort {
  return cur.key === col.key ? { key: col.key, desc: !cur.desc } : { key: col.key, desc: !!col.num }
}

/** Os filtros de dinheiro escrevem-se em milhares da moeda escolhida; o core recebe euros. */
export function currency() {
  return CURRENCIES.find((c) => c.key === getSettings().currency) ?? CURRENCIES[0]
}
export const toEuros = (thousands: number | null) => (thousands == null ? null : Math.round((thousands * 1000) / currency().rate))
export const fromEuros = (euros: number | null | undefined) => (euros == null ? null : Math.round((euros * currency().rate) / 1000))
