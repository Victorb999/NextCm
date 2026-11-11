import { useEffect, useState } from 'react'
import { api } from '../api'
import type { BoardView } from '../bindings/BoardView'
import type { ClubFinances } from '../bindings/ClubFinances'
import type { FinanceRow } from '../bindings/FinanceRow'
import { moneyFull, wage } from '../format'
import { useGame } from '../game'
import { PanelTitle } from './Screen'

// Finanças de um clube, como a tela do CM (Ver ▾ Finanças, Receitas, Despesas): cada rubrica
// em Este Mês, Último Mês, Este Ano (a época) e Último Ano; o Balanço e os Fundos Transferências.

export type FinanceView = 'finances' | 'income' | 'expenses'

const COLUMNS = ['Este Mês', 'Último Mês', 'Este Ano', 'Último Ano']

/** "€1.234.567", ou "(€1.234.567)" a vermelho nas perdas. */
function Amount({ v, tone }: { v: number; tone: string }) {
  if (v < 0) return <span className="text-[#ff5050]">({moneyFull(-v)})</span>
  return <span className={tone}>{moneyFull(v)}</span>
}

function Row({ row, tone, big }: { row: FinanceRow; tone: string; big?: boolean }) {
  return (
    <div className="grid h-[22px] grid-cols-[minmax(13rem,1.6fr)_repeat(4,1fr)] items-center px-4">
      <span className={`truncate font-semibold ${big ? 'text-[17px] text-[#ff7ad9]' : 'text-[17px] text-white'}`}>{row.label}</span>
      {row.amounts.map((v, i) => (
        <span key={i} className="text-[13px] tabular-nums">
          <Amount v={v} tone={tone} />
        </span>
      ))}
    </div>
  )
}

function Line({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid h-[22px] grid-cols-[minmax(13rem,1.6fr)_4fr] items-center px-4">
      <span className="text-[17px] font-semibold text-white">{label}</span>
      <span className="text-[13px] text-cm-yellow">{value}</span>
    </div>
  )
}

export default function FinancesPanel({ clubId, view, mine }: { clubId: number; view: FinanceView; mine: boolean }) {
  const game = useGame()
  const [data, setData] = useState<ClubFinances | null>(null)
  const [board, setBoard] = useState<BoardView | null>(null)
  const date = game.info?.date

  useEffect(() => {
    if (!date) return
    api.clubFinances(clubId).then(setData, () => setData(null))
    if (mine) api.boardView().then(setBoard, () => setBoard(null))
  }, [clubId, date, mine])

  if (!data) return null
  const title = view === 'income' ? 'Receitas' : view === 'expenses' ? 'Despesas' : 'Finanças'
  return (
    <div className="pb-2">
      <PanelTitle title={title} />
      <div className="py-1">
        <div className="mx-4 mb-3 grid h-7 grid-cols-[minmax(13rem,1.6fr)_repeat(4,1fr)] items-end border-b border-cm-orange pb-0.5">
          <span className="text-[17px] font-semibold text-cm-orange">Rúbrica</span>
          {COLUMNS.map((c) => (
            <span key={c} className="text-[13px] text-cm-orange">
              {c}
            </span>
          ))}
        </div>
        {view === 'finances' ? (
          <>
            <Row row={data.totalIncome} tone="text-cm-yellow" />
            <Row row={data.totalExpenses} tone="text-cm-yellow" />
            <div className="h-5" />
            <Row row={data.result} tone="text-cm-green" />
            <div className="h-5" />
            <div className="grid h-[22px] grid-cols-[minmax(13rem,1.6fr)_4fr] items-center px-4">
              <span className="text-[17px] font-semibold text-white">Balanço</span>
              <span className="text-[13px] tabular-nums">
                <Amount v={data.cash} tone="text-cm-green" />
              </span>
            </div>
            <div className="grid h-[22px] grid-cols-[minmax(13rem,1.6fr)_4fr] items-center px-4">
              <span className="text-[17px] font-semibold text-white">Fundos Transferências</span>
              <span className="text-[13px] tabular-nums text-cm-green">{moneyFull(data.funds)}</span>
            </div>
            {/* Fair-play financeiro (não há no CM): o resultado das últimas 3 épocas contra o limite */}
            <div className="grid h-[22px] grid-cols-[minmax(13rem,1.6fr)_4fr] items-center px-4">
              <span className="text-[17px] font-semibold text-white">Fair-play (3 épocas)</span>
              <span className="text-[13px] tabular-nums">
                <Amount v={data.fairPlayResult} tone="text-cm-green" />
                <span className="text-cm-yellow">
                  {' '}
                  (perdas até {moneyFull(-data.fairPlayLimit)}){data.fairPlayBanned ? ' — proibido de contratar' : ''}
                </span>
              </span>
            </div>
            {/* O que o CM mostra noutras telas da Direcção: fica aqui, no clube do treinador */}
            {mine && board && (
              <div className="mt-4">
                <Line label="Folha salarial" value={wage(data.wageBill, true)} />
                <Line label="Limite salarial" value={`${wage(board.wageBudget, true)} (máximo ${wage(board.maxWage, true)} por jogador)`} />
                <Line label="Confiança da Direcção" value={`${board.confidenceLabel} (${board.confidence}%)`} />
                <Line label="Objectivo" value={`${board.goal.charAt(0).toUpperCase()}${board.goal.slice(1)} (${board.target}º lugar)`} />
              </div>
            )}
          </>
        ) : (
          <>
            {(view === 'income' ? data.income : data.expenses).map((r) => (
              <Row key={r.label} row={r} tone="text-cm-yellow" />
            ))}
            <div className="h-5" />
            <Row row={{ ...(view === 'income' ? data.totalIncome : data.totalExpenses), label: 'Total' }} tone="text-[#ff7ad9]" big />
          </>
        )}
      </div>
    </div>
  )
}
