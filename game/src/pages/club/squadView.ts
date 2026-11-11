import type { ReactNode } from 'react'
import type { SquadPlayer } from '../../bindings/SquadPlayer'
import { type FinanceView } from '../../components/FinancesPanel'
import { type MenuOption } from '../../components/Screen'
import { type Spot } from '../../selection'
import { type SquadView } from '../../squad'

export const TABS = [
  { key: 'squad', label: 'Plantel' },
  { key: 'transfers', label: 'Transferências' },
  { key: 'next', label: 'Próximo Jogo' },
  { key: 'fixtures', label: 'Calendário' },
  { key: 'info', label: 'Finanças & Info' },
]

export type View = SquadView | 'staff'

export type Criterion =
  | 'name'
  | 'position'
  | 'number'
  | 'form'
  | 'morale'
  | 'condition'
  | 'nation'
  | 'age'
  | 'caps'
  | 'intlGoals'
  | 'wage'
  | 'contract'
  | 'value'
  | 'goals'
  | 'conceded'
  | 'assists'
  | 'rating'

// O Vêr do CM
export const VIEWS: MenuOption<View>[] = [
  { key: 'traditional', label: 'Tradicional' },
  { key: 'contract', label: 'Contrato' },
  { key: 'selection', label: 'Selecção' },
  { key: 'stats', label: 'Estatísticas' },
  { key: 'more', label: 'Mais Dados' },
  { key: 'attributes', label: 'Atributos' },
  { key: 'others', label: 'Outros' },
  'separator',
  { key: 'penalties', label: 'Marcadores Penalties' },
  'separator',
  { key: 'staff', label: 'Staff' },
]

// O Critério do CM (a ordem da lista)
export const CRITERIA: MenuOption<Criterion>[] = [
  { key: 'name', label: 'Nome' },
  { key: 'position', label: 'Posição' },
  { key: 'number', label: 'Numeração' },
  { key: 'form', label: 'Forma' },
  { key: 'morale', label: 'Moral' },
  { key: 'condition', label: 'Condição' },
  { key: 'nation', label: 'Nacionalidade' },
  { key: 'age', label: 'Idade' },
  { key: 'caps', label: 'Internacion.' },
  { key: 'intlGoals', label: 'Golos Intern.' },
  { key: 'wage', label: 'Salário Base' },
  { key: 'contract', label: 'Termo Contrato' },
  { key: 'value', label: 'Valor' },
  'separator',
  { key: 'goals', label: 'Golos' },
  { key: 'conceded', label: 'Golos Sofridos' },
  { key: 'assists', label: 'Assistências' },
  { key: 'rating', label: 'Class. Média' },
]

export const criterionLabel = (c: Criterion) =>
  CRITERIA.find((o): o is { key: Criterion; label: string } => o !== 'separator' && o.key === c)?.label ?? ''

/** Uma linha do plantel: [número] [lugar/Int] Nome ... posição */
export type Entry = {
  id: number
  number: ReactNode
  badge: ReactNode
  name: string
  detail: string
  spot?: Spot
  player?: SquadPlayer
}

export type Bind = { 'data-spot': string; onMouseDown: (e: React.MouseEvent) => void }

/** "D D" -> "DD", "M C" -> "MC": a sigla curta da função, como nas casas do plantel do CM. */
export const shortRole = (role: string) => role.replace(' ', '')

export const byNullsLast = (a: number | null, b: number | null) => (a ?? 999) - (b ?? 999)

/** Finanças pela palavra, como o CM (o banco guarda o saldo em euros). */
export function financesLabel(cash: number | null) {
  if (cash === null) return '-'
  if (cash >= 50_000_000) return 'Rico'
  if (cash >= 10_000_000) return 'Seguro'
  if (cash >= 1_000_000) return 'Razoável'
  if (cash >= 0) return 'Apertado'
  return 'Em dívida'
}

export const INFO_VIEWS: { key: 'info' | FinanceView; label: string }[] = [
  { key: 'info', label: 'Informação Geral' },
  { key: 'finances', label: 'Finanças' },
  { key: 'income', label: 'Receitas' },
  { key: 'expenses', label: 'Despesas' },
]
