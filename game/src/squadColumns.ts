import type { ReactNode } from 'react'
import type { SquadAttributes } from './bindings/SquadAttributes'
import type { SquadMore } from './bindings/SquadMore'
import type { SquadPlayer } from './bindings/SquadPlayer'
import { cmDate, money, wage } from './format'
import { ATTRIBUTE_GROUPS, FIXED_SORTS, formAverage, moraleRank, type AttributeGroup, type Sortable, type SquadView } from './squad'

// As colunas das vistas em tabela do Vêr do plantel (contrato, selecção, estatísticas...),
// cada uma com o valor por que se ordena ao clicar no cabeçalho.

export type Column = Sortable & {
  label: string
  width: string
  align?: 'left' | 'right' | 'center'
  cell: (p: SquadPlayer) => ReactNode
}

/** O que as vistas novas precisam do core: o Mais Dados e os atributos de cada jogador. */
export type TableExtra = {
  more?: Map<number, SquadMore>
  attributes?: Map<number, SquadAttributes['attributes']>
  group?: AttributeGroup
}

export type TableView = Exclude<SquadView, 'traditional'>

const rating = (r: number | null) => (r === null ? '-' : r.toFixed(2).replace('.', ','))

/** "(3)" a seguir aos jogos: quantos foram a sair do banco. */
const apps = (p: SquadPlayer) => (p.apps ? `${p.apps - p.subs}${p.subs ? ` (${p.subs})` : ''}` : '-')

const value = (p: SquadPlayer) => (p.free ? 'Livre' : money(p.value))
const num = (v: number | undefined) => (v === undefined ? '-' : v)
const form = (p: SquadPlayer, sep: string) => (p.form.length ? p.form.map((f) => Math.round(f)).join(sep) : '-')

// Colunas que se repetem
const VALUE: Column = { label: 'Valor', width: 'w-24', align: 'right', cell: value, sort: (p) => (p.free ? null : p.value), desc: true }
const POSITION: Column = { label: 'Posição', width: 'w-28', cell: (p) => p.position, sort: (p) => p.positionOrder }
const RATING: Column = { label: 'Cla M', width: 'w-16', align: 'right', cell: (p) => rating(p.rating), sort: (p) => p.rating, desc: true }
const CONDITION: Column = {
  label: 'Cond.',
  width: 'w-16',
  align: 'right',
  cell: (p) => (p.condition === null ? '-' : `${p.condition}%`),
  sort: (p) => p.condition,
  desc: true,
}

/** As colunas de uma vista; as do Mais Dados e Atributos dependem do que veio do core. */
export function columnsFor(view: TableView, extra: TableExtra): Column[] {
  if (view === 'more') {
    const m = (p: SquadPlayer) => extra.more?.get(p.id)
    const stat = (label: string, f: (s: SquadMore) => number): Column => ({
      label,
      width: 'w-12',
      align: 'right',
      cell: (p) => num(m(p) && f(m(p)!)),
      sort: (p) => (m(p) ? f(m(p)!) : null),
      desc: true,
    })
    return [
      stat('Sof', (s) => s.conceded),
      { label: 'Pens', width: 'w-12', align: 'right', cell: () => '-' },
      stat('Amr', (s) => s.yellows),
      stat('Verm', (s) => s.reds),
      stat('V', (s) => s.wins),
      stat('D', (s) => s.losses),
      stat('GlsE', (s) => s.teamFor),
      stat('SofE', (s) => s.teamAgainst),
      { ...RATING, cell: (p) => rating(m(p)?.rating ?? null), sort: (p) => m(p)?.rating },
      VALUE,
    ]
  }
  if (view === 'attributes') {
    const group = ATTRIBUTE_GROUPS.find((g) => g.key === extra.group) ?? ATTRIBUTE_GROUPS[0]
    return [
      ...group.columns.map(
        ([key, label]): Column => ({
          label,
          width: 'w-11',
          align: 'right',
          cell: (p) => extra.attributes?.get(p.id)?.[key] ?? '-',
          sort: (p) => extra.attributes?.get(p.id)?.[key],
          desc: true,
        }),
      ),
      VALUE,
    ]
  }
  return COLUMNS[view]
}

const COLUMNS: Record<Exclude<TableView, 'more' | 'attributes'>, Column[]> = {
  contract: [
    { label: 'Situação', width: 'w-28', cell: (p) => p.role ?? '-', sort: (p) => p.role },
    {
      label: 'Salário Base',
      width: 'w-28',
      align: 'right',
      cell: (p) => (p.free ? '-' : wage(p.wage) || '-'),
      sort: (p) => (p.free ? null : p.wage),
      desc: true,
    },
    {
      label: 'Termo Contrato',
      width: 'w-32',
      align: 'center',
      cell: (p) => (p.free ? '-' : cmDate(p.contractExpires)),
      sort: (p) => (p.free ? null : p.contractExpires),
    },
    { label: 'Cláusulas', width: 'w-32', cell: (p) => (p.clauses.length ? p.clauses.join(', ') : '-') },
    { ...VALUE, width: 'w-28' },
  ],
  selection: [
    POSITION,
    { label: 'Forma', width: 'w-32', cell: (p) => form(p, ' '), sort: formAverage, desc: true },
    { label: 'Moral', width: 'w-28', cell: (p) => p.morale ?? '-', sort: moraleRank },
    CONDITION,
    { label: 'Pres', width: 'w-20', align: 'right', cell: apps, sort: (p) => p.apps || null, desc: true },
    RATING,
    { ...VALUE, width: 'w-28' },
  ],
  stats: [
    POSITION,
    { label: 'Jogos', width: 'w-20', align: 'right', cell: apps, sort: (p) => p.apps || null, desc: true },
    { label: 'Golos', width: 'w-16', align: 'right', cell: (p) => p.goals || '-', sort: (p) => p.goals || null, desc: true },
    { label: 'Assist', width: 'w-16', align: 'right', cell: (p) => p.assists || '-', sort: (p) => p.assists || null, desc: true },
    { label: 'HJ', width: 'w-12', align: 'right', cell: (p) => p.mom || '-', sort: (p) => p.mom || null, desc: true },
    RATING,
  ],
  others: [
    { label: 'Nat.', width: 'w-14', cell: (p) => p.nationCode ?? '-', sort: (p) => p.nationCode },
    { label: 'Idade', width: 'w-14', align: 'right', cell: (p) => p.age ?? '-', sort: (p) => p.age },
    { label: 'Ints', width: 'w-12', align: 'right', cell: (p) => p.caps, sort: (p) => p.caps, desc: true },
    { label: 'Golos', width: 'w-14', align: 'right', cell: (p) => p.intlGoals, sort: (p) => p.intlGoals, desc: true },
    { label: 'Forma', width: 'w-28', cell: (p) => form(p, '-'), sort: formAverage, desc: true },
    { label: 'Moral', width: 'w-24', cell: (p) => p.morale ?? '-', sort: moraleRank },
    { ...CONDITION, width: 'w-14' },
    VALUE,
  ],
  penalties: [POSITION, { label: 'Ordem', width: 'w-16', align: 'right', cell: (p) => p.penaltyOrder ?? '-', sort: (p) => p.penaltyOrder }],
}

/** A coluna de um cabeçalho da vista: as das tabelas do Vêr ou as fixas (Nome, Nº...). */
export function headerColumn(view: SquadView | 'staff', extra: TableExtra, label: string): Sortable | undefined {
  const table = view !== 'traditional' && view !== 'staff' ? columnsFor(view, extra) : []
  return table.find((c) => c.label === label) ?? FIXED_SORTS[label]
}
