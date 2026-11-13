// Formatos de exibição no estilo do CM 01/02.

import { CURRENCIES, type NonEmpty, WAGES, getSettings } from './settings'

/** "2007-07-13" -> [2007, 7, 13] (o que faltar fica 0). */
export function ymd(iso: string): [number, number, number] {
  const [y = 0, m = 0, d = 0] = iso.split('-').map(Number)
  return [y, m, d]
}

/** "2007-07-13" -> "13.7.2007" */
export function cmDate(iso: string | null | undefined): string {
  if (!iso) return '-'
  const [y, m, d] = ymd(iso)
  return `${d}.${m}.${y}`
}

/** Valor em euros (como no banco) na moeda escolhida: 11000 -> "€11K", 2500000 -> "€2.5M". */
export function money(v: number | null | undefined): string {
  if (!v) return '-'
  const c = CURRENCIES.find((c) => c.key === getSettings().currency) ?? CURRENCIES[0]
  const sign = v < 0 ? '-' : ''
  const a = Math.round(Math.abs(v) * c.rate)
  if (a >= 1_000_000) return `${sign}${c.symbol}${(a / 1_000_000).toFixed(a % 1_000_000 >= 50_000 ? 1 : 0)}M`
  if (a >= 10_000) return `${sign}${c.symbol}${Math.round(a / 1_000)}K`
  // Os salários pequenos por inteiro: "€1.600"
  if (a >= 1_000) return `${sign}${c.symbol}${a.toLocaleString('pt-BR')}`
  return `${sign}${c.symbol}${a}`
}

/** Valor por inteiro na moeda escolhida, como nas Finanças do CM: 25337195 -> "€25.337.195". */
export function moneyFull(v: number): string {
  const c = CURRENCIES.find((c) => c.key === getSettings().currency) ?? CURRENCIES[0]
  const a = Math.round(Math.abs(v) * c.rate)
  return `${v < 0 ? '-' : ''}${c.symbol}${a.toLocaleString('pt-BR')}`
}

/** Salário semanal do banco no período escolhido: "€10K p/s" ou, com `long`, "€43K por mês". */
export function wage(weekly: number | null | undefined, long = false): string {
  if (!weekly) return '-'
  const w = WAGES.find((w) => w.key === getSettings().wages) ?? WAGES[0]
  return `${money(weekly * w.factor)} ${long ? w.long : w.suffix}`
}

/** "de Arrascaeta, Giorgian" -> "de Arrascaeta, G" (como no plantel do CM); nomes comuns ficam iguais. */
export function shortListName(listName: string): string {
  const i = listName.indexOf(', ')
  return i < 0 ? listName : `${listName.slice(0, i)}, ${listName[i + 2] ?? ''}`
}

const WEEKDAYS_SHORT: NonEmpty<string> = ['Dom', 'Seg', 'Ter', 'Qua', 'Qui', 'Sex', 'Sáb']

/** "2025-08-02" -> "Sáb 2.8.2025" (calendários e jogos). */
export function matchDate(iso: string): string {
  const [y, m, d] = ymd(iso)
  return `${WEEKDAYS_SHORT[new Date(y, m - 1, d).getDay()]} ${d}.${m}.${y}`
}

const WEEKDAYS_LONG = ['Domingo', 'Segunda', 'Terça', 'Quarta', 'Quinta', 'Sexta', 'Sábado']
const MONTHS_SHORT = ['Jan', 'Fev', 'Mar', 'Abr', 'Mai', 'Jun', 'Jul', 'Ago', 'Set', 'Out', 'Nov', 'Dez']
const MONTHS_LONG = [
  'Janeiro',
  'Fevereiro',
  'Março',
  'Abril',
  'Maio',
  'Junho',
  'Julho',
  'Agosto',
  'Setembro',
  'Outubro',
  'Novembro',
  'Dezembro',
]

/** "2036-06-30" -> "Jun 2036" (fim de contrato). */
export function monthYear(iso: string): string {
  if (!iso) return '-'
  const [y, m] = ymd(iso)
  return `${MONTHS_SHORT[m - 1]} ${y}`
}

/** "2025-08-02" -> "Sáb 2 Ago" (calendários do CM). */
export function shortDate(iso: string): string {
  const [y, m, d] = ymd(iso)
  return `${WEEKDAYS_SHORT[new Date(y, m - 1, d).getDay()]} ${d} ${MONTHS_SHORT[m - 1]}`
}

/** "2025-08-02" -> "Sábado 2 Agosto", com "(hoje)"/"(amanhã)" em relação a `today`. */
export function longDate(iso: string, today?: string): string {
  const [y, m, d] = ymd(iso)
  const base = `${WEEKDAYS_LONG[new Date(y, m - 1, d).getDay()]} ${d} ${MONTHS_LONG[m - 1]}`
  if (!today) return base
  const days = Math.round((Date.UTC(y, m - 1, d) - Date.parse(`${today}T00:00:00Z`)) / 86_400_000)
  // Como no Próximo Jogo do CM: "Domingo, 19 de Dezembro (8 dias)"
  const long = `${WEEKDAYS_LONG[new Date(y, m - 1, d).getDay()]}, ${d} de ${MONTHS_LONG[m - 1]}`
  return days === 0 ? `${long} (hoje)` : days === 1 ? `${long} (amanhã)` : days > 1 ? `${long} (${days} dias)` : long
}

/** A parte do dia, como na data do CM: manhã, tarde e noite. */
export function partLabel(part: number): string {
  return ['MNH', 'TAR', 'NTE'][part] ?? ''
}

/** Penáltis de uma eliminatória empatada: " (4-3 g.p.)". */
export function pensText(f: { homePens: number | null; awayPens: number | null }): string {
  return f.homePens !== null && f.awayPens !== null ? ` (${f.homePens}-${f.awayPens} g.p.)` : ''
}

/** Moeda escolhida nas Definições (símbolo e câmbio a partir do euro). */
export function currency() {
  return CURRENCIES.find((c) => c.key === getSettings().currency) ?? CURRENCIES[0]
}

/** Período dos salários escolhido nas Definições (fator a partir do semanal). */
export function wagePeriod() {
  return WAGES.find((w) => w.key === getSettings().wages) ?? WAGES[0]
}

/** Passo "redondo" para os botões − / + de um valor: ~5% dele. */
export function moneyStep(v: number): number {
  const target = Math.max(Math.abs(v) * 0.05, 100)
  const p = 10 ** Math.floor(Math.log10(target))
  return [1, 2, 5, 10].map((m) => m * p).find((s) => s >= target) ?? p * 10
}

/** Os valores redondos das listas de dinheiro (na moeda mostrada): 0, 100, 150, 200, 250... 1 bilião. */
export const MONEY_LADDER: number[] = [0].concat(
  ...[2, 3, 4, 5, 6, 7, 8].map((e) => [1, 1.5, 2, 2.5, 3, 4, 5, 6, 7.5].map((m) => m * 10 ** e)),
  [1e9],
)

/** As opções da lista à volta de `v`: de um vigésimo a vinte vezes, com o próprio `v`. */
export function moneyOptions(v: number): number[] {
  const near = MONEY_LADDER.filter((x) => (v <= 0 ? x <= 1_000_000 : x >= v / 20 && x <= v * 20))
  return [...new Set([0, ...near, v])].sort((a, b) => a - b)
}

/** O passo do − / + pela ordem de grandeza: €100 até 10 mil, mil até 100 mil, 10 mil até 1
 * milhão, 100 mil até 10 milhões e 500 mil acima. */
function plusStep(v: number): number {
  if (v < 10_000) return 100
  if (v < 100_000) return 1_000
  if (v < 1_000_000) return 10_000
  if (v < 10_000_000) return 100_000
  return 500_000
}

/** O valor seguinte (dir = 1) ou anterior (dir = -1), arredondado ao passo (600.002 + → 610.000). */
export function moneyNext(v: number, dir: 1 | -1): number {
  if (dir > 0) {
    const s = plusStep(v)
    return (Math.floor(v / s) + 1) * s
  }
  if (v <= 0) return 0
  const s = plusStep(v - 1)
  return Math.max(0, (Math.ceil(v / s) - 1) * s)
}

/** "Palmeiras" -> "PA", "Real Madrid" -> "RM": o emblema do clube (placar, Tácticas). */
export function initials(name: string): string {
  const words = name.split(/\s+/).filter((w) => /^\p{Lu}/u.test(w))
  const [a, b] = words
  return (a && b ? (a[0] ?? '') + (b[0] ?? '') : name.slice(0, 2)).toUpperCase()
}

/** As qualificações do treinador no Novo Jogo, pela ordem do core (0 a 4). */
export const LICENSES = ['Nenhuma', 'Licença C', 'Licença B', 'Licença A', 'Licença Pro']

/** A experiência passada do treinador (Novo Jogo), pela ordem do core (`EXPERIENCES`). */
export const EXPERIENCES = ['Nenhuma', 'Jogador amador', 'Jogador semi-profissional', 'Jogador profissional', 'Jogador internacional']

/**
 * Um texto do core (notícias, avisos) na moeda e no período de salários das Definições. O core
 * escreve sempre em euros ("€2,6M", "€266K", "€1.600") e os salários "por semana"; aqui cada
 * valor passa pela moeda e "€266K por semana" fica, por exemplo, "€1.2M por mês".
 */
export function localText(text: string): string {
  const s = getSettings()
  if (s.currency === CURRENCIES[0].key && s.wages === WAGES[0].key) return text
  return text.replace(/€(\d{1,3}(?:\.\d{3})+|\d+(?:,\d+)?)([KM]?)( por semana)?/g, (_, n: string, unit: string, weekly?: string) => {
    const base = n.includes('.') ? Number(n.replace(/\./g, '')) : Number(n.replace(',', '.'))
    const eur = base * (unit === 'M' ? 1_000_000 : unit === 'K' ? 1_000 : 1)
    return weekly ? wage(eur, true) : money(eur)
  })
}
