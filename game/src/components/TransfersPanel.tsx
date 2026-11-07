import { useEffect, useState } from 'react'
import { api } from '../api'
import type { TransferItem } from '../bindings/TransferItem'
import { money } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'
import { Empty, Link, PanelTitle } from './Screen'

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
  const th = 'bg-[#3d1a73] px-3 text-left font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-cm-yellow'
  return (
    <>
      <PanelTitle title={title} />
      {items.length === 0 ? (
        <Empty className="py-2">Nenhuma.</Empty>
      ) : (
        // Como as Transferências do mundo: cabeçalho roxo e zebra
        <table className="w-full table-fixed text-[14px]">
          <thead>
            <tr className="h-7">
              <th className={`${th} w-32`}>Data</th>
              <th className={th}>Jogador</th>
              <th className={`${th} w-56`}>{title === 'Entradas' ? 'De' : 'Para'}</th>
              <th className={`${th} w-36 !text-right`}>Valor</th>
            </tr>
          </thead>
          <tbody>
            {items.map((t, i) => {
              const club = other(t)
              return (
                <tr key={`${t.personId}-${t.date}-${i}`} data-person={t.personId} className="h-6">
                  <td className="px-3 text-white">{shortDate(t.date)}</td>
                  <td className="truncate px-3">
                    <Link onClick={() => nav.go({ kind: 'person', id: t.personId })}>{t.name}</Link>
                  </td>
                  <td className="truncate px-3 text-cm-magenta">
                    {club ? <Link onClick={() => nav.go({ kind: 'club', id: club.id })}>{club.shortName}</Link> : <span className="text-white">-</span>}
                  </td>
                  <td className="px-3 text-right font-bold tabular-nums text-cm-yellow">{feeText(t)}</td>
                </tr>
              )
            })}
          </tbody>
        </table>
      )}
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
    <div className="min-h-0 flex-1 overflow-auto pb-3">
      <div className="flex h-8 items-center gap-8 px-4 font-mono text-[12px] uppercase tracking-wider text-[#86869a]">
        <span className="text-cm-orange">Últimos 12 meses</span>
        <span className="text-white">
          Gastos: <span className="text-cm-yellow">{spent ? money(spent) : '€0'}</span>
        </span>
        <span className="text-white">
          Receitas: <span className="text-cm-yellow">{received ? money(received) : '€0'}</span>
        </span>
      </div>
      <List title="Entradas" items={incoming} other={(t) => t.from} />
      <List title="Saídas" items={outgoing} other={(t) => t.to} />
    </div>
  )
}
