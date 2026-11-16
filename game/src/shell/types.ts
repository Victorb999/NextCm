import type { Fixture } from '../bindings/Fixture'
import type { StandingRow } from '../bindings/StandingRow'
import type { StartSeason } from '../bindings/StartSeason'

/** O que o ecrã do dia de jogo precisa: o jogo, os jogos do dia e a tabela de antes. */
export type MatchDayData = { fixture: Fixture; dayGames: Fixture[]; table: StandingRow[] }

/** Data mostrada sem jogo aberto: a da temporada que o Novo Jogo escolhe por omissão (Europa 26/27). */
export const START_DATE = '2026-07-01'

/** O primeiro dia de cada temporada do Novo Jogo (os textos do `LeaguesStep`), para a barra do topo. */
export const SEASON_START: Record<StartSeason, string> = { patch: '2025-07-01', europe: '2026-07-01', brazil: '2026-01-01' }

export const WEEKDAYS = ['Domingo', 'Segunda', 'Terça', 'Quarta', 'Quinta', 'Sexta', 'Sábado']
