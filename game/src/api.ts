import { invoke, isTauri } from '@tauri-apps/api/core'
import type { ClubProfile } from './bindings/ClubProfile'
import type { ClubTactics } from './bindings/ClubTactics'
import type { CompetitionListItem } from './bindings/CompetitionListItem'
import type { CompetitionProfile } from './bindings/CompetitionProfile'
import type { Fixture } from './bindings/Fixture'
import type { GameInfo } from './bindings/GameInfo'
import type { NewGame } from './bindings/NewGame'
import type { NewsItem } from './bindings/NewsItem'
import type { SaveSummary } from './bindings/SaveSummary'
import type { Selection } from './bindings/Selection'
import type { NationListItem } from './bindings/NationListItem'
import type { NationProfile } from './bindings/NationProfile'
import type { NationOption } from './bindings/NationOption'
import type { PersonProfile } from './bindings/PersonProfile'
import type { SearchQuery } from './bindings/SearchQuery'
import type { SearchResult } from './bindings/SearchResult'
import type { StandingRow } from './bindings/StandingRow'

export const inTauri = isTauri()

/** Fora do app desktop (só em `npm run dev`), os comandos vão para a ponte HTTP
 * `cargo run -p nextcm-core --example dev_bridge`, via proxy do Vite. */
export const hasBackend = inTauri || import.meta.env.DEV

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (inTauri) return invoke<T>(cmd, args)
  const res = await fetch(`/invoke/${cmd}`, { method: 'POST', body: JSON.stringify(args ?? {}) })
  const data = await res.json()
  if (!res.ok) throw data
  return data as T
}

/** Listas que não mudam na Fase 2: carregadas uma vez, voltar à tela fica instantâneo. */
function once<T>(load: () => Promise<T>): () => Promise<T> {
  let cached: Promise<T> | null = null
  return () => {
    cached ??= load().catch((e) => {
      cached = null
      throw e
    })
    return cached
  }
}

export const api = {
  searchPeople: (query: SearchQuery) => call<SearchResult>('search_people', { query }),
  listNations: once(() => call<NationOption[]>('list_nations')),
  getPerson: (id: number) => call<PersonProfile>('get_person', { id }),
  getClub: (id: number) => call<ClubProfile>('get_club', { id }),
  getClubTactics: (id: number) => call<ClubTactics>('get_club_tactics', { id }),
  listCompetitions: once(() => call<CompetitionListItem[]>('list_competitions')),
  getCompetition: (id: number) => call<CompetitionProfile>('get_competition', { id }),
  listNationsFull: once(() => call<NationListItem[]>('list_nations_full')),
  getNation: (id: number) => call<NationProfile>('get_nation', { id }),

  // Jogo em curso (Fase 3)
  gameInfo: () => call<GameInfo | null>('game_info'),
  listSaves: () => call<SaveSummary[]>('list_saves'),
  newGame: (game: NewGame) => call<GameInfo>('new_game', { game }),
  loadGame: (file: string) => call<GameInfo>('load_game', { file }),
  saveGame: () => call<GameInfo>('save_game'),
  closeGame: () => call<null>('close_game'),
  advanceDay: () => call<GameInfo>('advance_day'),
  /** Avança até o próximo evento (jogo do treinador, notícia...). */
  continueGame: () => call<GameInfo>('continue_game'),
  /** Classificação real; vazia sem jogo ou numa competição parada. */
  leagueTable: (id: number) => call<StandingRow[]>('league_table', { id }),
  listFixtures: (id: number) => call<Fixture[]>('list_fixtures', { id }),
  clubFixtures: (id: number) => call<Fixture[]>('club_fixtures', { id }),
  listNews: () => call<NewsItem[]>('list_news'),
  readNews: (id: number) => call<GameInfo>('read_news', { id }),
  previewTactics: (selection: Selection) => call<ClubTactics>('preview_tactics', { selection }),
  setTactics: (selection: Selection) => call<ClubTactics>('set_tactics', { selection }),
  togglePlayer: (selection: Selection, player: number) => call<ClubTactics>('toggle_player', { selection, player }),
}
