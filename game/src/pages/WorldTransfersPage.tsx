import { ArrowLeftRight } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import type { TransferItem } from '../bindings/TransferItem'
import type { WorldTransfers } from '../bindings/WorldTransfers'
import { BottomBar, Empty, Link, ScreenHeader } from '../components/Screen'
import { cmDate, money } from '../format'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'

// "Transferências" do menu do treinador, como no CM: os negócios do mundo mês a mês, com a
// data, o jogador, o clube de origem, o destino e o valor (os mais caros primeiro).

const MONTHS = ['Janeiro', 'Fevereiro', 'Março', 'Abril', 'Maio', 'Junho', 'Julho', 'Agosto', 'Setembro', 'Outubro', 'Novembro', 'Dezembro']

/** O mês antes (-1) ou depois (+1) de "2025-08". */
function shift(month: string, d: number) {
  const [y, m] = month.split('-').map(Number)
  const t = y * 12 + (m - 1) + d
  return `${Math.floor(t / 12)}-${String((t % 12) + 1).padStart(2, '0')}`
}

function feeText(t: TransferItem) {
  if (t.kind === 'loan') return 'Empréstimo'
  return t.fee > 0 ? money(t.fee) : 'Livre'
}

export default function WorldTransfersPage() {
  const nav = useNav()
  const game = useGame()
  useEscBack()
  const [month, setMonth] = useState(() => (game.info?.date ?? '2025-07-01').slice(0, 7))
  const [page, setPage] = useState(0)
  const [data, setData] = useState<WorldTransfers | null>(null)
  useEffect(() => {
    api.worldTransfers(month, page).then(setData, () => setData(null))
  }, [month, page])
  const pages = Math.max(1, Math.ceil((data?.total ?? 0) / 200))
  const [y, m] = month.split('-').map(Number)
  const go = (d: number) => {
    setMonth((mo) => shift(mo, d))
    setPage(0)
  }

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader
        icon={<ArrowLeftRight size={22} />}
        eyebrow={data ? `${data.total} negócios` : undefined}
        title="Transferências"
        right={
          <>
            <button className="cm-purple h-7 w-9 px-0" onClick={() => go(-1)}>
              ◀
            </button>
            <span className="w-36 text-center font-title text-[17px] font-bold text-cm-yellow">
              {MONTHS[m - 1]} {y}
            </span>
            <button className="cm-purple h-7 w-9 px-0" onClick={() => go(1)}>
              ▶
            </button>
          </>
        }
      />
      <div className="flex shrink-0 items-center justify-end gap-3 px-3 pb-1">
        {pages > 1 && (
          <span className="flex items-center gap-1 text-[13px] text-white">
            <button className="cm-purple h-5 w-6" disabled={page === 0} onClick={() => setPage((p) => p - 1)}>
              ‹
            </button>
            Pág {page + 1}/{pages}
            <button className="cm-purple h-5 w-6" disabled={page + 1 >= pages} onClick={() => setPage((p) => p + 1)}>
              ›
            </button>
          </span>
        )}
      </div>
      <div className="mx-2 mb-1 min-h-0 flex-1 overflow-auto rounded-md bg-[#1a1b21]">
        <table className="w-full table-fixed text-[14px]">
          <thead className="sticky top-0">
            <tr className="h-7">
              {['Data', 'Jogador', 'De', 'Para', 'Valor'].map((h) => (
                <th key={h} className="bg-[#3d1a73] px-3 text-left font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-cm-yellow">
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data?.items.map((t, i) => (
              <tr key={`${t.personId}-${i}`} className="h-6">
                <td className="px-2 text-white">{cmDate(t.date)}</td>
                <td className="truncate px-2">
                  <Link onClick={() => nav.go({ kind: 'person', id: t.personId })}>{t.name}</Link>
                </td>
                <td className="truncate px-2 text-cm-magenta">
                  {t.from ? <Link onClick={() => nav.go({ kind: 'club', id: t.from!.id })}>{t.from.shortName}</Link> : 'Sem clube'}
                </td>
                <td className="truncate px-2 text-cm-magenta">
                  {t.to ? <Link onClick={() => nav.go({ kind: 'club', id: t.to!.id })}>{t.to.shortName}</Link> : 'Sem clube'}
                </td>
                <td className="px-2 font-bold text-cm-yellow">{feeText(t)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {data && data.items.length === 0 && <Empty>Sem transferências neste mês.</Empty>}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
