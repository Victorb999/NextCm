import type { DealTerms } from './bindings/DealTerms'
import type { LoanTerms } from './bindings/LoanTerms'
import type { TransferView } from './bindings/TransferView'
import { money } from './format'

// Condições iniciais dos diálogos do mercado e a frase da direção (aba Transferência,
// menu Acção e mensagens).

export const NO_TERMS: DealTerms = { fee: 0, months: 0, sellOn: 0, appsBonus: 0, appsNeeded: 10, public: false, exchange: null }
export const LOAN_TERMS: LoanTerms = { months: 0, wageShare: 100, monthlyFee: 0, optionFee: 0 }

/** A frase da direção sobre o orçamento, como no CM. */
export function budgetText(view: TransferView) {
  return view.budgetNote ?? `A sua direcção autoriza transferências até ${money(view.budget) || '€0'}.`
}
