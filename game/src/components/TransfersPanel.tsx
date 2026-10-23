import { useEffect, useState } from 'react'
import { api } from '../api'
import type { TransferItem } from '../bindings/TransferItem'
import { money } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'
import { Link } from './Screen'

// Transferências do clube nos últimos 12 meses, como a tela do CM: entradas em cima, saídas
// em baixo; cada linha com a data, o jogador, o outro clube e o valor.

const MONTHS = ['Jan', 'Fev', 'Mar', 'Abr', 'Mai', 'Jun', 'Jul', 'Ago', 'Set', 'Out', 'Nov', 'Dez']

/** "2026-07-14" -> "14 Jul 2026". */
const shortDate = (iso: string) => {
  const [y, m, d] = iso.split('-').map(Number)
  return `${d} ${MONTHS[m - 1]} ${y}`
}

/** O valor pelo tipo do negócio. */
function feeText(t: TransferItem) {
  if (t.kind === 'loan') return 'Empréstimo'
  if (t.kind === 'return') return 'Fim de empréstimo'
  if (t.kind === 'release') return 'Dispensado'
  return t.fee > 0 ? money(t.fee) : 'Livre'
}

function List({ title, items, other }: { title: string; items: TransferItem[]; other: (t: TransferItem) => TransferItem['from'] }) {
  const nav = useNav()
  return (
    <>
      <div className="mb-1 mt-2 inline-block border-b-2 border-cm-orange font-title text-[20px] font-bold text-cm-orange">{title}</div>
      {items.length === 0 && <div className="px-2 text-[15px] text-cm-magenta">Nenhuma.</div>}
      {items.map((t, i) => {
        const club = other(t)
        return (
          <div key={`${t.personId}-${t.date}-${i}`} className="flex h-[24px] items-center gap-3 border-b border-cm-line bg-black/40 px-2 text-[15px]">
            <span className="w-28 shrink-0 text-white">{shortDate(t.date)}</span>
            <span className="min-w-0 flex-1 truncate">
              <Link onClick={() => nav.go({ kind: 'person', id: t.personId })}>{t.name}</Link>
            </span>
            <span className="w-56 shrink-0 truncate">
              {club ? <Link onClick={() => nav.go({ kind: 'club', id: club.id })}>{club.shortName}</Link> : <span className="text-white">-</span>}
            </span>
            <span className="w-36 shrink-0 text-right tabular-nums text-cm-yellow">{feeText(t)}</span>
          </div>
        )
      })}
    </>
  )
}

export default function TransfersPanel({ clubId }: { clubId: number }) {
  const game = useGame()
  const [items, setItems] = useState<TransferItem[]>([])
  const date = game.info?.date

  useEffect(() => {
    api.clubTransfers(clubId).then(setItems, () => setItems([]))
  }, [clubId, date])

  const incoming = items.filter((t) => t.to?.id === clubId)
  const outgoing = items.filter((t) => t.from?.id === clubId)
  const spent = incoming.reduce((s, t) => s + (t.kind === 'transfer' ? t.fee : 0), 0)
  const received = outgoing.reduce((s, t) => s + (t.kind === 'transfer' ? t.fee : 0), 0)
  return (
    <div className="min-h-0 flex-1 overflow-auto px-4 pb-3">
      <div className="py-1.5 text-center font-title text-xl font-bold text-cm-yellow">Transferências (últimos 12 meses)</div>
      <div className="flex gap-8 px-2 text-[15px]">
        <span className="text-white">
          Gastos: <span className="text-cm-yellow">{money(spent) || '€0'}</span>
        </span>
        <span className="text-white">
          Receitas: <span className="text-cm-yellow">{money(received) || '€0'}</span>
        </span>
      </div>
      <List title="Entradas" items={incoming} other={(t) => t.from} />
      <List title="Saídas" items={outgoing} other={(t) => t.to} />
    </div>
  )
}
