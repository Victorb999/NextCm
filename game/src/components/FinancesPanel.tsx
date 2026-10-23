import { useEffect, useState } from 'react'
import { api } from '../api'
import type { ClubFinances } from '../bindings/ClubFinances'
import type { SeasonBooks } from '../bindings/SeasonBooks'
import { money, wage } from '../format'
import { useGame } from '../game'

// Finanças do clube do treinador, como a tela do CM: saldo e folha em cima, receitas e
// despesas da época em duas colunas, com a época anterior a um clique.

function Column({ title, lines, total }: { title: string; lines: SeasonBooks['income']; total: number }) {
  return (
    <div className="min-w-0 flex-1">
      <div className="mb-1 border-b-2 border-cm-orange font-title text-[20px] font-bold text-cm-orange">{title}</div>
      {lines.map((l) => (
        <div key={l.label} className="flex h-[24px] items-center justify-between border-b border-cm-line bg-black/40 px-2 text-[15px]">
          <span className="text-white">{l.label}</span>
          <span className="tabular-nums text-cm-yellow">{l.amount ? money(l.amount) : '-'}</span>
        </div>
      ))}
      <div className="flex h-[26px] items-center justify-between bg-black/70 px-2 text-[15px] font-bold">
        <span className="text-white">Total</span>
        <span className="tabular-nums text-cm-yellow">{money(total)}</span>
      </div>
    </div>
  )
}

export default function FinancesPanel({ clubId }: { clubId: number }) {
  const game = useGame()
  const [data, setData] = useState<ClubFinances | null>(null)
  const [last, setLast] = useState(false)
  const date = game.info?.date

  useEffect(() => {
    if (date) api.clubFinances(clubId).then(setData, () => setData(null))
  }, [clubId, date])

  if (!data) return null
  const books = last && data.lastSeason ? data.lastSeason : data.thisSeason
  const result = books.totalIncome - books.totalExpenses
  const row = (label: string, value: string, red = false) => (
    <div className="flex min-h-[26px] items-baseline gap-4">
      <span className="w-60 shrink-0 font-title text-[21px] leading-tight text-white">{label}</span>
      <span className={`flex-1 text-[15px] ${red ? 'text-[#ff5050]' : 'text-cm-yellow'}`}>{value}</span>
    </div>
  )
  return (
    <div className="pb-2">
      <div className="py-1.5 text-center font-title text-xl font-bold text-cm-yellow">Finanças</div>
      {row('Saldo', money(data.cash) || '€0', data.cash < 0)}
      {row('Folha salarial', wage(data.wageBill, true))}
      {row('Receitas por época', money(data.expectedRevenue))}
      <div className="mt-2 flex items-center gap-1">
        <button className={`cm-purple h-6 w-44 ${!last ? '!text-cm-yellow' : ''}`} onClick={() => setLast(false)}>
          Esta época
        </button>
        <button className={`cm-purple h-6 w-44 ${last ? '!text-cm-yellow' : ''}`} disabled={!data.lastSeason} onClick={() => setLast(true)}>
          Época anterior
        </button>
        <span className="ml-2 font-title text-lg font-bold text-cm-yellow">Época {books.season}</span>
      </div>
      <div className="mt-2 flex gap-4">
        <Column title="Receitas" lines={books.income} total={books.totalIncome} />
        <Column title="Despesas" lines={books.expenses} total={books.totalExpenses} />
      </div>
      <div className="mt-1 flex h-[28px] items-center justify-between bg-black/70 px-2 font-title text-[19px] font-bold">
        <span className="text-white">Resultado da época</span>
        <span className={result < 0 ? 'text-[#ff5050]' : 'text-cm-green'}>{result < 0 ? `-${money(-result)}` : money(result) || '€0'}</span>
      </div>
    </div>
  )
}
