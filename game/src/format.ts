// Formatos de exibição no estilo do CM 01/02.

import { CURRENCIES, WAGES, getSettings } from './settings'

/** "2007-07-13" -> "13.7.2007" */
export function cmDate(iso: string | null | undefined): string {
  if (!iso) return '-'
  const [y, m, d] = iso.split('-').map(Number)
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

const WEEKDAYS_SHORT = ['Dom', 'Seg', 'Ter', 'Qua', 'Qui', 'Sex', 'Sáb']

/** "2025-08-02" -> "Sáb 2.8.2025" (calendários e jogos). */
export function matchDate(iso: string): string {
  const [y, m, d] = iso.split('-').map(Number)
  return `${WEEKDAYS_SHORT[new Date(y, m - 1, d).getDay()]} ${d}.${m}.${y}`
}

const WEEKDAYS_LONG = ['Domingo', 'Segunda', 'Terça', 'Quarta', 'Quinta', 'Sexta', 'Sábado']
const MONTHS_SHORT = ['Jan', 'Fev', 'Mar', 'Abr', 'Mai', 'Jun', 'Jul', 'Ago', 'Set', 'Out', 'Nov', 'Dez']
const MONTHS_LONG = ['Janeiro', 'Fevereiro', 'Março', 'Abril', 'Maio', 'Junho', 'Julho', 'Agosto', 'Setembro', 'Outubro', 'Novembro', 'Dezembro']

/** "2036-06-30" -> "Jun 2036" (fim de contrato). */
export function monthYear(iso: string): string {
  if (!iso) return '-'
  const [y, m] = iso.split('-').map(Number)
  return `${MONTHS_SHORT[m - 1]} ${y}`
}

/** "2025-08-02" -> "Sáb 2 Ago" (calendários do CM). */
export function shortDate(iso: string): string {
  const [y, m, d] = iso.split('-').map(Number)
  return `${WEEKDAYS_SHORT[new Date(y, m - 1, d).getDay()]} ${d} ${MONTHS_SHORT[m - 1]}`
}

/** "2025-08-02" -> "Sábado 2 Agosto", com "(hoje)"/"(amanhã)" em relação a `today`. */
export function longDate(iso: string, today?: string): string {
  const [y, m, d] = iso.split('-').map(Number)
  const base = `${WEEKDAYS_LONG[new Date(y, m - 1, d).getDay()]} ${d} ${MONTHS_LONG[m - 1]}`
  if (!today) return base
  const days = Math.round((Date.UTC(y, m - 1, d) - Date.parse(`${today}T00:00:00Z`)) / 86_400_000)
  return days === 0 ? `${base} (hoje)` : days === 1 ? `${base} (amanhã)` : base
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

/** "Palmeiras" -> "PA", "Real Madrid" -> "RM": o emblema do clube (placar, Tácticas). */
export function initials(name: string): string {
  const words = name.split(/\s+/).filter((w) => /^\p{Lu}/u.test(w))
  return (words.length > 1 ? words[0][0] + words[1][0] : name.slice(0, 2)).toUpperCase()
}
