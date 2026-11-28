import type { DealTerms } from './bindings/DealTerms'
import type { LoanTerms } from './bindings/LoanTerms'
import type { TransferView } from './bindings/TransferView'
import { money } from './format'

// Condições iniciais dos diálogos do mercado e a frase da direção (aba Transferência,
// menu Acção e mensagens).

export const NO_TERMS: DealTerms = {
  fee: 0,
  months: 0,
  sellOn: 0,
  appsBonus: 0,
  appsNeeded: 10,
  capsBonus: 0,
  capsNeeded: 5,
  public: false,
  exchange: null,
  firm: false,
  deadline: 0,
}
export const LOAN_TERMS: LoanTerms = { months: 0, wageShare: 100, monthlyFee: 0, optionFee: 0 }

export const INSTALMENTS: [number, string][] = [
  [0, '-'],
  [6, '6 meses'],
  [12, '12 meses'],
  [24, '24 meses'],
  [36, '36 meses'],
]
export const SELL_ON: [number, string][] = Array.from({ length: 11 }, (_, i) => [i * 5, i ? `${i * 5}%` : '-'])
export const LOAN_LENGTH: [number, string][] = [
  [0, 'Fim da temporada'],
  [6, '6 meses'],
  [12, '1 ano'],
]
export const WAGE_SHARE: [number, string][] = [0, 25, 50, 75, 100].map((v) => [v, `${v}%`])
/** A taxa mensal do empréstimo, em % do salário do mês. */
export const FEE_PCT = [0, 10, 25, 50, 75, 100, 150, 200]
/** O prazo da decisão (dias): o clube decide quando quiser, amanhã ou numa semana. */
export const DEADLINES: [number, string][] = [
  [0, 'Não definido'],
  [1, 'Amanhã'],
  [7, 'Uma semana'],
]

/** 10 → 20 → ... → 50 → 10 jogos. */
export const nextGames = (n: number) => (n >= 50 ? 10 : n + 10)
export const nextCaps = (n: number) => (n >= 25 ? 5 : n + 5)

// As condições de uma proposta anterior, para a "Nova Proposta" abrir com elas no perfil
let pendingTerms: { id: number; deal?: DealTerms; loan?: LoanTerms } | null = null
export const setPendingTerms = (id: number, deal?: DealTerms, loan?: LoanTerms) => {
  pendingTerms = { id, deal, loan }
}
/** As condições guardadas para este jogador (e esquece-as). */
export function takePendingTerms(id: number) {
  const p = pendingTerms
  pendingTerms = null
  return p?.id === id ? p : null
}

/** A frase da direção sobre o orçamento, como no CM. */
export function budgetText(view: TransferView) {
  // money(0) dá "-": aqui é um valor na frase
  const budget = view.budgetNote ?? `A sua diretoria autoriza transferências até ${view.budget ? money(view.budget) : '€0'}.`
  return `Saldo do clube: ${view.cash ? money(view.cash) : '€0'}. ${budget}`
}
