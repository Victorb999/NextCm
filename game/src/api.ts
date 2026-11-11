import { invoke, isTauri } from '@tauri-apps/api/core'
import type { BoardConfidence } from './bindings/BoardConfidence'
import type { ContractExtras } from './bindings/ContractExtras'
import type { FindResult } from './bindings/FindResult'
import type { MatchAnalysis } from './bindings/MatchAnalysis'
import type { MatchReview } from './bindings/MatchReview'
import type { BoardRequest } from './bindings/BoardRequest'
import type { BoardView } from './bindings/BoardView'
import type { DealTerms } from './bindings/DealTerms'
import type { DisciplineOptions } from './bindings/DisciplineOptions'
import type { LineupCheck } from './bindings/LineupCheck'
import type { LoanTerms } from './bindings/LoanTerms'
import type { MatchSheet } from './bindings/MatchSheet'
import type { TrainingPlan } from './bindings/TrainingPlan'
import type { TrainingView } from './bindings/TrainingView'
import type { WorldTransfers } from './bindings/WorldTransfers'
import type { CareerClub } from './bindings/CareerClub'
import type { ContinueStep } from './bindings/ContinueStep'
import type { CreationProgress } from './bindings/CreationProgress'
import type { LastLineup } from './bindings/LastLineup'
import type { ManagerStats } from './bindings/ManagerStats'
import type { SquadAttributes } from './bindings/SquadAttributes'
import type { SquadMore } from './bindings/SquadMore'
import type { LeagueNation } from './bindings/LeagueNation'
import type { TalkLine } from './bindings/TalkLine'
import type { TalkReaction } from './bindings/TalkReaction'
import type { TalkView } from './bindings/TalkView'
import type { PressView } from './bindings/PressView'
import type { JobsView } from './bindings/JobsView'
import type { NewsLink } from './bindings/NewsLink'
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
import type { SquadRoleView } from './bindings/SquadRoleView'
import type { EditorClub } from './bindings/EditorClub'
import type { EditorHit } from './bindings/EditorHit'
import type { EditorCommand } from './bindings/EditorCommand'
import type { EditorPlayer } from './bindings/EditorPlayer'
import type { EditorProfile } from './bindings/EditorProfile'
import type { EditorProfileView } from './bindings/EditorProfileView'
import type { EditorValue } from './bindings/EditorValue'
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
import type { ReserveLineup } from './bindings/ReserveLineup'
import type { ScoutList } from './bindings/ScoutList'
import type { SearchQuery } from './bindings/SearchQuery'
import type { SearchResult } from './bindings/SearchResult'
import type { Selection } from './bindings/Selection'
import type { SavedFile } from './bindings/SavedFile'
import type { SquadNumbers } from './bindings/SquadNumbers'
import type { StageView } from './bindings/StageView'
import type { StandingRow } from './bindings/StandingRow'
import type { TransferItem } from './bindings/TransferItem'
import type { ClubMoves } from './bindings/ClubMoves'
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
  /** O passo da criação do jogo, enquanto o `newGame` corre (barra do carregamento). */
  creationProgress: () => call<CreationProgress>('creation_progress'),
  /** Um dia do Continuar Jogo, com os resultados do dia (a tela repete até `stop`). */
  continueStep: (idle: number) => call<ContinueStep>('continue_step', { idle }),
  /** Antes do dia de jogo: o que falta ou o que confirmar na equipa. */
  lineupCheck: () => call<LineupCheck>('lineup_check'),
  pressView: (id: number) => call<PressView>('press_view', { id }),
  /** As respostas na conferência de imprensa, uma por pergunta. */
  answerPress: (id: number, answers: number[]) => call<string>('answer_press', { id, answers }),
  skipPress: (id: number) => call<string>('skip_press', { id }),
  setNickname: (id: number, nickname: string) => call<null>('set_nickname', { id, nickname }),
  setNote: (id: number, text: string) => call<null>('set_note', { id, text }),
  /** A pergunta antes de dispensar, com a indemnização. */
  releaseQuestion: (id: number) => call<string>('release_question', { id }),
  releasePlayer: (id: number) => call<string>('release_player', { id }),
  /** As competições com jogos acabados hoje (os resultados depois do jogo do treinador). */
  dayCompetitions: () => call<number[]>('day_competitions'),
  /** "Transferências": os negócios do mundo num mês ("2025-08"). */
  worldTransfers: (month: string, page: number, kind: string | null) => call<WorldTransfers>('world_transfers', { month, page, kind }),
  resign: () => call<string>('resign'),
  /** As nações do ecrã "Escolher Campeonato(s)". */
  leagueNations: () => call<LeagueNation[]>('league_nations'),
  /** O painel "Palestra" do jogo do treinador agora. */
  talkView: () => call<TalkView>('talk_view'),
  /** `person` = null fala à equipa toda. */
  giveTalk: (key: string, tone: string, person: number | null) => call<TalkReaction[]>('give_talk', { key, tone, person }),
  giveTalks: (tone: string, lines: TalkLine[]) => call<TalkReaction[]>('give_talks', { tone, lines }),
  retireManager: () => call<string>('retire_manager'),
  holiday: (days: number) => call<GameInfo>('holiday', { days }),
  managerCareer: () => call<CareerClub[]>('manager_career'),
  managerStats: () => call<ManagerStats>('manager_stats'),
  /** O Último Jogo das Tácticas: a escalação do último jogo do clube (`null` sem jogos). */
  lastLineup: (id: number) => call<LastLineup | null>('last_lineup', { id }),
  /** O Vêr Mais Dados do plantel (`kind`: '' jogos oficiais, 'Liga', 'Taça', 'Continentais'). */
  squadMore: (id: number, kind: string) => call<SquadMore[]>('squad_more', { id, kind }),
  /** O Vêr Atributos do plantel e dos Treinos (tapados como no perfil). */
  squadAttributes: (id: number) => call<SquadAttributes[]>('squad_attributes', { id }),
  /** Gravar com outro nome; `overwrite` substitui um ficheiro com esse nome. */
  saveAs: (name: string, overwrite: boolean) => call<GameInfo>('save_as', { name, overwrite }),
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
  trainingView: () => call<TrainingView>('training_view'),
  matchSheet: (id: number) => call<MatchSheet>('match_sheet', { id }),
  setTraining: (plan: TrainingPlan) => call<TrainingView>('set_training', { plan }),
  /** A direção do clube do treinador: expectativa, confiança e orçamentos. */
  boardView: () => call<BoardView>('board_view'),
  boardConfidence: () => call<BoardConfidence>('board_confidence'),
  boardRequests: () => call<BoardRequest[]>('board_requests'),
  /** Um pedido à direção; devolve a resposta. */
  boardRequest: (key: string) => call<string>('board_request', { key }),
  leagueRequests: () => call<BoardRequest[]>('league_requests'),
  disciplineOptions: (id: number) => call<DisciplineOptions>('discipline_options', { id }),
  /** Castigo (índice em `actions`) e razão; devolve a reação do jogador. */
  discipline: (id: number, action: number, reason: string) => call<string>('discipline', { id, action, reason }),
  /** "praise" ou "criticise"; devolve a reação do jogador. */
  pressComment: (id: number, kind: string) => call<string>('press_comment', { id, kind }),
  /** Despromover para as reservas (true) ou promover ao plantel principal. */
  setReserve: (id: number, reserve: boolean) => call<string>('set_reserve', { id, reserve }),
  /** O onze das reservas para o próximo jogo delas e os escolhidos pelo treinador. */
  reserveLineup: () => call<ReserveLineup>('reserve_lineup'),
  setReservePicks: (ids: number[]) => call<ReserveLineup>('set_reserve_picks', { ids }),
  /** "Dar Folga" a quem tem saudades de casa. */
  giveLeave: (id: number) => call<string>('give_leave', { id }),
  jobsView: (nation: number | null) => call<JobsView>('jobs_view', { nation }),
  /** "Concorrer ao Lugar"; devolve a frase com a data da resposta. */
  applyJob: (id: number) => call<string>('apply_job', { id }),
  /** Os nomes de jogadores e clubes que uma notícia cita (para os mostrar como ligações). */
  newsLinks: (id: number) => call<NewsLink[]>('news_links', { id }),
  /** Adicionar (true) ou Retirar da Lista Preferencial. */
  setShortlist: (id: number, on: boolean) => call<null>('set_shortlist', { id, on }),
  /** Uma questão à Liga; devolve a resposta. */
  leagueRequest: (key: string) => call<string>('league_request', { key }),
  /** Aba Transferência do perfil de um jogador (jogo em curso). */
  transferView: (id: number) => call<TransferView>('transfer_view', { id }),
  makeOffer: (id: number, terms: DealTerms) => call<TransferView>('make_offer', { id, terms }),
  /** Editor de jogos salvos (save fechado): procurar jogadores, abrir um e gravar os atributos. */
  editorSearch: (file: string, query: string) => call<EditorHit[]>('editor_search', { file, query }),
  editorPlayer: (file: string, id: number) => call<EditorPlayer>('editor_player', { file, id }),
  editorSetAttributes: (file: string, id: number, values: EditorValue[]) =>
    call<EditorPlayer>('editor_set_attributes', { file, id, values }),
  editorProfile: (file: string, id: number) => call<EditorProfileView>('editor_profile', { file, id }),
  editorSetProfile: (file: string, id: number, profile: EditorProfile) =>
    call<EditorProfileView>('editor_set_profile', { file, id, profile }),
  editorSearchClubs: (file: string, query: string) => call<EditorHit[]>('editor_search_clubs', { file, query }),
  editorSquad: (file: string, clubId: number) => call<EditorHit[]>('editor_squad', { file, clubId }),
  /** O "Custom Command" do editor: os comandos e aplicá-los a uma lista de jogadores (devolve quantos mudaram). */
  editorCommands: () => call<EditorCommand[]>('editor_commands'),
  editorCommand: (file: string, ids: number[], commands: string[], firstTeam: boolean) =>
    call<number>('editor_command', { file, ids, commands, firstTeam }),
  editorClub: (file: string, id: number) => call<EditorClub>('editor_club', { file, id }),
  editorSetClub: (file: string, club: EditorClub) => call<EditorClub>('editor_set_club', { file, club }),
  squadRoleView: (id: number) => call<SquadRoleView>('squad_role_view', { id }),
  setSquadRole: (id: number, role: string) => call<string>('set_squad_role', { id, role }),
  offerToClubs: (id: number, loan: boolean, fee: number, excludeRivals: boolean) =>
    call<string>('offer_to_clubs', { id, loan, fee, excludeRivals }),
  exchangeCandidates: (id: number) => call<number[]>('exchange_candidates', { id }),
  askLoan: (id: number, terms: LoanTerms) => call<TransferView>('ask_loan', { id, terms }),
  /** Exercer a opção de compra de um emprestado. */
  exerciseOption: (id: number) => call<TransferView>('exercise_option', { id }),
  /** "Pedir Mais" com condições numa proposta recebida. */
  askTerms: (id: number, terms: DealTerms) => call<OfferView>('ask_terms', { id, terms }),
  /** Funções no clube e o que o jogador pede pela função escolhida (sem ela, pela que espera). */
  contractTerms: (id: number, role: string | null) => call<ContractTerms>('contract_terms', { id, role }),
  /** Proposta de contrato: renovação ou contratação depois de o clube aceitar. */
  negotiate: (id: number, role: string, wage: number, bonus: number, years: number, extras: ContractExtras) =>
    call<ContractReply>('negotiate', { id, role, wage, bonus, years, extras }),
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
  /** O Procurar: clubes, nações, competições, jogadores e staff pelo nome. */
  find: (text: string) => call<FindResult>('find', { text }),
  /** Entradas e saídas de um clube nos últimos 12 meses. */
  clubTransfers: (id: number) => call<TransferItem[]>('club_transfers', { id }),
  /** Chamar de volta um jogador emprestado pelo clube do treinador. */
  recallLoan: (id: number) => call<string>('recall_loan', { id }),
  /** A aba Transferências do clube: a vista do menu Ver ("in", "out", "future"...) e a época. */
  clubMoves: (id: number, view: string, season: number | null) => call<ClubMoves>('club_moves', { id, view, season }),
  clubFixtures: (id: number) => call<Fixture[]>('club_fixtures', { id }),
  /** Os jogos das reservas do clube do treinador (véspera de cada jogo do campeonato). */
  reserveFixtures: (id: number) => call<Fixture[]>('reserve_fixtures', { id }),
  listNews: () => call<NewsItem[]>('list_news'),
  readNews: (id: number) => call<GameInfo>('read_news', { id }),
  previewTactics: (selection: Selection) => call<ClubTactics>('preview_tactics', { selection }),
  setTactics: (selection: Selection) => call<ClubTactics>('set_tactics', { selection }),
  togglePlayer: (selection: Selection, player: number) => call<ClubTactics>('toggle_player', { selection, player }),
  /** As tácticas gravadas na pasta `tactics` (Carregar Táctica). */
  listTactics: () => call<SavedFile[]>('list_tactics'),
  saveTactic: (selection: Selection, name: string) => call<SavedFile>('save_tactic', { selection, name }),
  /** As listas de jogadores gravadas (Carregar Lista); `slot` 0 = Lista Preferencial, 1–7 = olheiros. */
  listLists: () => call<SavedFile[]>('list_lists'),
  saveList: (slot: number, name: string) => call<SavedFile>('save_list', { slot, name }),
  loadList: (slot: number, file: string) => call<null>('load_list', { slot, file }),
  /** A escolha com a táctica do ficheiro; a tela aplica-a. */
  loadTactic: (selection: Selection, file: string) => call<Selection>('load_tactic', { selection, file }),

  // Jogo do treinador ao vivo (marco 3.4)
  liveMatch: () => call<LiveView>('live_match'),
  /** A Análise do Pré-Jogo: odds, homem-chave, previsão do jornal e marcos. */
  matchAnalysis: () => call<MatchAnalysis>('match_analysis'),
  /** O Pós-Jogo: o jornal a comparar com a previsão, os homens-chave e os marcos cumpridos. */
  matchReview: () => call<MatchReview>('match_review'),
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
