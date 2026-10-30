import type { ReactNode } from 'react'
import type { SquadPlayer } from '../bindings/SquadPlayer'
import { cmDate, money, wage } from '../format'
import { nameClass, squadName, type SquadView } from '../squad'
import { Link } from './Screen'

// As vistas em tabela do Vêr do plantel, como no CM: uma linha por jogador com Esc, Inf e
// Nome à esquerda e as colunas da vista (contrato, selecção, estatísticas, penáltis).

export type TableRow = { p: SquadPlayer; place: string | null }

type Column = { label: string; width: string; align?: 'left' | 'right' | 'center'; cell: (p: SquadPlayer) => ReactNode }

const rating = (r: number | null) => (r === null ? '-' : r.toFixed(2).replace('.', ','))

/** "(3)" a seguir aos jogos: quantos foram a sair do banco. */
const apps = (p: SquadPlayer) => (p.apps ? `${p.apps - p.subs}${p.subs ? ` (${p.subs})` : ''}` : '-')

const COLUMNS: Record<Exclude<SquadView, 'traditional'>, Column[]> = {
  contract: [
    { label: 'Situação', width: 'w-28', cell: (p) => p.role ?? '-' },
    { label: 'Salário Base', width: 'w-28', align: 'right', cell: (p) => (p.free ? '-' : wage(p.wage) || '-') },
    { label: 'Termo Contrato', width: 'w-32', align: 'center', cell: (p) => (p.free ? '-' : cmDate(p.contractExpires)) },
    { label: 'Cláusulas', width: 'w-32', cell: (p) => (p.clauses.length ? p.clauses.join(', ') : '-') },
    { label: 'Valor', width: 'w-28', align: 'right', cell: (p) => (p.free ? 'Livre' : money(p.value)) },
  ],
  selection: [
    { label: 'Posição', width: 'w-28', cell: (p) => p.position },
    {
      label: 'Forma',
      width: 'w-32',
      cell: (p) => (p.form.length ? p.form.map((f) => Math.round(f)).join(' ') : '-'),
    },
    { label: 'Moral', width: 'w-28', cell: (p) => p.morale ?? '-' },
    { label: 'Cond.', width: 'w-16', align: 'right', cell: (p) => (p.condition === null ? '-' : `${p.condition}%`) },
    { label: 'Pres', width: 'w-20', align: 'right', cell: apps },
    { label: 'Cla M', width: 'w-16', align: 'right', cell: (p) => rating(p.rating) },
    { label: 'Valor', width: 'w-28', align: 'right', cell: (p) => (p.free ? 'Livre' : money(p.value)) },
  ],
  stats: [
    { label: 'Posição', width: 'w-28', cell: (p) => p.position },
    { label: 'Jogos', width: 'w-20', align: 'right', cell: apps },
    { label: 'Golos', width: 'w-16', align: 'right', cell: (p) => p.goals || '-' },
    { label: 'Assist', width: 'w-16', align: 'right', cell: (p) => p.assists || '-' },
    { label: 'HJ', width: 'w-12', align: 'right', cell: (p) => p.mom || '-' },
    { label: 'Cla M', width: 'w-16', align: 'right', cell: (p) => rating(p.rating) },
  ],
  penalties: [
    { label: 'Posição', width: 'w-28', cell: (p) => p.position },
    { label: 'Ordem', width: 'w-16', align: 'right', cell: (p) => p.penaltyOrder ?? '-' },
  ],
}

const ALIGN = { left: 'text-left', right: 'text-right', center: 'text-center' }

/** A sigla da coluna Inf, nas cores do CM (lesão laranja, castigo vermelho). */
export function InfBadge({ info }: { info: string | null }) {
  if (!info) return <span className="text-[#5a5a66]">-</span>
  const colour = info === 'Les' ? 'bg-[#b8560c] text-white' : info === 'Sus' ? 'bg-[#9c0000] text-white' : 'text-[#4edea3]'
  return <span className={`px-0.5 font-bold ${colour}`}>{info}</span>
}

export default function SquadTable({
  view,
  rows,
  selected,
  onSelect,
  onOpen,
}: {
  view: Exclude<SquadView, 'traditional'>
  rows: TableRow[]
  selected: number
  onSelect: (i: number) => void
  onOpen: (id: number) => void
}) {
  const columns = COLUMNS[view]
  const head = 'font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-[#86869a]'
  return (
    <div className="overflow-hidden rounded-md bg-[#1a1b21]">
      <div className={`flex h-7 items-center gap-2 border-b border-cm-line bg-black/30 px-2 ${head}`}>
        <span className="w-10 text-center">Esc</span>
        <span className="w-10 text-center">Inf</span>
        <span className="min-w-28 flex-1">Nome</span>
        {columns.map((c) => (
          <span key={c.label} className={`${c.width} shrink-0 ${ALIGN[c.align ?? 'left']}`}>
            {c.label}
          </span>
        ))}
      </div>
      <div className="cm-zebra">
        {rows.map(({ p, place }, i) => (
          <div
            key={p.id}
            data-row={i}
            onClick={() => onSelect(i)}
            onDoubleClick={() => onOpen(p.id)}
            className={`flex h-6 items-center gap-2 px-2 ${i === selected ? '!bg-cm-sel' : ''}`}
          >
            <span className="w-10 shrink-0 text-center font-mono text-[12px] font-bold text-[#4edea3]">
              {place ?? <span className="text-[#5a5a66]">-</span>}
            </span>
            <span className="w-10 shrink-0 text-center font-mono text-[12px]">
              <InfBadge info={p.info} />
            </span>
            <span className={`min-w-28 flex-1 truncate text-[15px] font-semibold ${nameClass(p)}`}>
              <Link onClick={() => onOpen(p.id)}>{squadName(p)}</Link>
            </span>
            {columns.map((c) => (
              <span
                key={c.label}
                className={`${c.width} shrink-0 truncate font-mono text-[12.5px] ${ALIGN[c.align ?? 'left']} ${i === selected ? 'text-white' : 'text-cm-yellow'}`}
              >
                {c.cell(p)}
              </span>
            ))}
          </div>
        ))}
      </div>
    </div>
  )
}
