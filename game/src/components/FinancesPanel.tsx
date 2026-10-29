import { useEffect, useState } from 'react'
import { api } from '../api'
import type { BoardView } from '../bindings/BoardView'
import type { ClubFinances } from '../bindings/ClubFinances'
import type { SeasonBooks } from '../bindings/SeasonBooks'
import { money, wage } from '../format'
import { useGame } from '../game'
import { PanelTitle } from './Screen'

// Finanças do clube do treinador, como a tela do CM: saldo e folha em cima, receitas e
// despesas da época em duas colunas, com a época anterior a um clique.

function Column({ title, lines, total }: { title: string; lines: SeasonBooks['income']; total: number }) {
  return (
    <div className="min-w-0 flex-1 overflow-hidden rounded-md bg-black/20">
      <PanelTitle title={title} />
      <div className="cm-zebra">
        {lines.map((l) => (
          <div key={l.label} className="flex h-[26px] items-center justify-between px-3 text-[14px]">
            <span className="text-white">{l.label}</span>
            <span className="tabular-nums text-cm-yellow">{l.amount ? money(l.amount) : '-'}</span>
          </div>
        ))}
      </div>
      <div className="flex h-[27px] items-center justify-between bg-[#1d4d3a]/50 px-3 text-[14px] font-bold">
        <span className="text-[#4edea3]">Total</span>
        <span className="tabular-nums text-cm-yellow">{money(total)}</span>
      </div>
    </div>
  )
}

export default function FinancesPanel({ clubId }: { clubId: number }) {
  const game = useGame()
  const [data, setData] = useState<ClubFinances | null>(null)
  const [board, setBoard] = useState<BoardView | null>(null)
  const [last, setLast] = useState(false)
  const date = game.info?.date

  useEffect(() => {
    if (!date) return
    api.clubFinances(clubId).then(setData, () => setData(null))
    api.boardView().then(setBoard, () => setBoard(null))
  }, [clubId, date])

  if (!data) return null
  const books = last && data.lastSeason ? data.lastSeason : data.thisSeason
  const result = books.totalIncome - books.totalExpenses
  const row = (label: string, value: string, red = false) => (
    <div className="flex min-h-[27px] items-baseline gap-4 px-3 py-1">
      <span className="w-56 shrink-0 text-[15px] font-semibold text-white">{label}</span>
      <span className={`flex-1 text-[14px] ${red ? 'text-[#ff5050]' : 'text-cm-yellow'}`}>{value}</span>
    </div>
  )
  return (
    <div className="pb-2">
      <PanelTitle title="Finanças" />
      <div className="cm-zebra">
      {row('Saldo', money(data.cash) || '€0', data.cash < 0)}
      {row('Folha salarial', wage(data.wageBill, true))}
      {row('Receitas por época', money(data.expectedRevenue))}
      {board && (
        <>
          {row('Confiança da Direcção', `${board.confidenceLabel} (${board.confidence}%)`, board.confidence < 35)}
          {row('Objectivo', `${board.goal.charAt(0).toUpperCase()}${board.goal.slice(1)} (${board.target}º lugar)`)}
          {row('Orçamento', `${money(board.transferBudget) || '€0'} para transferências`)}
          {row('Limite salarial', `${wage(board.wageBudget, true)} (máximo ${wage(board.maxWage, true)} por jogador)`)}
        </>
      )}
      </div>
      <div className="mt-2 flex items-center gap-1 px-3">
        <button className={`cm-purple h-6 w-44 ${!last ? '!text-cm-yellow' : ''}`} onClick={() => setLast(false)}>
          Esta época
        </button>
        <button className={`cm-purple h-6 w-44 ${last ? '!text-cm-yellow' : ''}`} disabled={!data.lastSeason} onClick={() => setLast(true)}>
          Época anterior
        </button>
        <span className="ml-2 font-mono text-[13px] font-bold uppercase tracking-wider text-cm-orange">Época {books.season}</span>
      </div>
      <div className="mt-2 flex gap-2.5 px-3">
        <Column title="Receitas" lines={books.income} total={books.totalIncome} />
        <Column title="Despesas" lines={books.expenses} total={books.totalExpenses} />
      </div>
      <div className="mx-3 mt-2 flex h-[30px] items-center justify-between rounded-md bg-black/30 px-3 text-[15px] font-bold">
        <span className="text-white">Resultado da época</span>
        <span className={result < 0 ? 'text-[#ff5050]' : 'text-cm-green'}>{result < 0 ? `-${money(-result)}` : money(result) || '€0'}</span>
      </div>
    </div>
  )
}
