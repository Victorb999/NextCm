import type { Fixture } from '../bindings/Fixture'
import type { StandingRow } from '../bindings/StandingRow'

/** O que o ecrã do dia de jogo precisa: o jogo, os jogos do dia e a tabela de antes. */
export type MatchDayData = { fixture: Fixture; dayGames: Fixture[]; table: StandingRow[] }

/** Data mostrada sem jogo aberto (início do patch CLZ 25-26). */
export const START_DATE = '2025-07-01'

export const WEEKDAYS = ['Domingo', 'Segunda', 'Terça', 'Quarta', 'Quinta', 'Sexta', 'Sábado']
