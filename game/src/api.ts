import { invoke, isTauri } from '@tauri-apps/api/core'
import type { BoardConfidence } from './bindings/BoardConfidence'
import type { BoardView } from './bindings/BoardView'
import type { ClubFinances } from './bindings/ClubFinances'
import type { ClubListItem } from './bindings/ClubListItem'
import type { ClubProfile } from './bindings/ClubProfile'
import type { ClubTactics } from './bindings/ClubTactics'
import type { CompetitionListItem } from './bindings/CompetitionListItem'
import type { CompetitionProfile } from './bindings/CompetitionProfile'
import type { Fixture } from './bindings/Fixture'
import type { GameInfo } from './bindings/GameInfo'
import type { ContractReply } from './bindings/ContractReply'
import type { ContractTerms } from './bindings/ContractTerms'
import type { Instructions } from './bindings/Instructions'
import type { LiveView } from './bindings/LiveView'
import type { MatchPreview } from './bindings/MatchPreview'
import type { NationListItem } from './bindings/NationListItem'
import type { NationOption } from './bindings/NationOption'
import type { NationProfile } from './bindings/NationProfile'
import type { NewGame } from './bindings/NewGame'
import type { NumberAssign } from './bindings/NumberAssign'
import type { NewsItem } from './bindings/NewsItem'
import type { OfferView } from './bindings/OfferView'
import type { PersonProfile } from './bindings/PersonProfile'
import type { SaveSummary } from './bindings/SaveSummary'
import type { ReportStaff } from './bindings/ReportStaff'
import type { ScoutList } from './bindings/ScoutList'
import type { SearchQuery } from './bindings/SearchQuery'
import type { SearchResult } from './bindings/SearchResult'
import type { Selection } from './bindings/Selection'
import type { SquadNumbers } from './bindings/SquadNumbers'
import type { StageView } from './bindings/StageView'
import type { StandingRow } from './bindings/StandingRow'
import type { TransferItem } from './bindings/TransferItem'
import type { TransferView } from './bindings/TransferView'

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
  deleteSave: (file: string) => call<null>('delete_save', { file }),
  matchPreview: (id: number) => call<MatchPreview>('match_preview', { id }),
  saveGame: () => call<GameInfo>('save_game'),
  closeGame: () => call<null>('close_game'),
  advanceDay: () => call<GameInfo>('advance_day'),
  /** Avança até o próximo evento (jogo do treinador, notícia...). */
  continueGame: () => call<GameInfo>('continue_game'),
  /** Classificação real; vazia sem jogo ou numa competição parada. */
  leagueTable: (id: number) => call<StandingRow[]>('league_table', { id }),
  listFixtures: (id: number) => call<Fixture[]>('list_fixtures', { id }),
  /** Fases de uma competição viva: classificações, grupos e eliminatórias. */
  competitionStages: (id: number) => call<StageView[]>('competition_stages', { id }),
  /** Finanças de um clube no jogo em curso (saldo, folha, contas da época). */
  clubFinances: (id: number) => call<ClubFinances>('club_finances', { id }),
  /** A tela "Números Plantel" do clube do treinador. */
  squadNumbers: () => call<SquadNumbers>('squad_numbers'),
  /** O botão Auto: números para quem não tem (sem gravar). */
  autoSquadNumbers: (numbers: NumberAssign[]) => call<NumberAssign[]>('auto_squad_numbers', { numbers }),
  setSquadNumbers: (numbers: NumberAssign[]) => call<SquadNumbers>('set_squad_numbers', { numbers }),
  /** A direção do clube do treinador: expectativa, confiança e orçamentos. */
  boardView: () => call<BoardView>('board_view'),
  boardConfidence: () => call<BoardConfidence>('board_confidence'),
  /** Aba Transferência do perfil de um jogador (jogo em curso). */
  transferView: (id: number) => call<TransferView>('transfer_view', { id }),
  makeOffer: (id: number, fee: number) => call<TransferView>('make_offer', { id, fee }),
  askLoan: (id: number) => call<TransferView>('ask_loan', { id }),
  /** Funções no clube e o que o jogador pede pela função escolhida (sem ela, pela que espera). */
  contractTerms: (id: number, role: string | null) => call<ContractTerms>('contract_terms', { id, role }),
  /** Proposta de contrato: renovação ou contratação depois de o clube aceitar. */
  negotiate: (id: number, role: string, wage: number, bonus: number, years: number) =>
    call<ContractReply>('negotiate', { id, role, wage, bonus, years }),
  setListing: (id: number, transfer: boolean, loan: boolean, asking: number | null) =>
    call<TransferView>('set_listing', { id, transfer, loan, asking }),
  offerView: (id: number) => call<OfferView>('offer_view', { id }),
  /** Responder a uma proposta: accept, reject, counter (com fee), pay, withdraw. */
  answerOffer: (id: number, key: string, fee: number | null) => call<OfferView>('answer_offer', { id, key, fee }),
  /** "Observar Jogador": devolve a frase com a data do relatório. */
  scoutPlayer: (id: number) => call<string>('scout_player', { id }),
  scoutList: (slot: number) => call<ScoutList>('scout_list', { slot }),
  /** Quem pode fazer o relatório: "medical" ou "fitness". */
  reportStaff: (kind: string) => call<ReportStaff[]>('report_staff', { kind }),
  /** Pede os relatórios; devolve quantos chegaram às mensagens. */
  staffReport: (id: number, kind: string, staff: number[]) => call<number>('staff_report', { id, kind, staff }),
  assignScout: (slot: number, kind: string, target: number | null) => call<ScoutList>('assign_scout', { slot, kind, target }),
  clearScout: (slot: number) => call<ScoutList>('clear_scout', { slot }),
  searchClubs: (text: string) => call<ClubListItem[]>('search_clubs', { text }),
  /** Entradas e saídas de um clube nos últimos 12 meses. */
  clubTransfers: (id: number) => call<TransferItem[]>('club_transfers', { id }),
  clubFixtures: (id: number) => call<Fixture[]>('club_fixtures', { id }),
  listNews: () => call<NewsItem[]>('list_news'),
  readNews: (id: number) => call<GameInfo>('read_news', { id }),
  previewTactics: (selection: Selection) => call<ClubTactics>('preview_tactics', { selection }),
  setTactics: (selection: Selection) => call<ClubTactics>('set_tactics', { selection }),
  togglePlayer: (selection: Selection, player: number) => call<ClubTactics>('toggle_player', { selection, player }),

  // Jogo do treinador ao vivo (marco 3.4)
  liveMatch: () => call<LiveView>('live_match'),
  /** Joga até ao minuto dado (no máximo 90). */
  liveStep: (minute: number) => call<LiveView>('live_step', { minute }),
  liveSubstitute: (out: number, inn: number) => call<LiveView>('live_substitute', { out, inn }),
  liveSwap: (a: number, b: number) => call<LiveView>('live_swap', { a, b }),
  liveInstructions: (instructions: Instructions) => call<LiveView>('live_instructions', { instructions }),
  /** Tácticas de um lado do jogo (0 casa, 1 fora), como a tela as mostra. */
  liveTactics: (side: number) => call<ClubTactics>('live_tactics', { side }),
  /** Como ficaria a equipa do treinador com esta escolha (sem mexer no jogo). */
  livePreviewTactics: (selection: Selection) => call<ClubTactics>('live_preview_tactics', { selection }),
  /** O Ok das Tácticas a meio do jogo. */
  liveSetTactics: (selection: Selection) => call<LiveView>('live_set_tactics', { selection }),
  /** Acaba o jogo e fecha o dia (resultados e notícias). */
  liveFinish: () => call<GameInfo>('live_finish'),
}
