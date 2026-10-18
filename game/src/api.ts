import { invoke, isTauri } from '@tauri-apps/api/core'
import type { ClubProfile } from './bindings/ClubProfile'
import type { CompetitionListItem } from './bindings/CompetitionListItem'
import type { CompetitionProfile } from './bindings/CompetitionProfile'
import type { NationListItem } from './bindings/NationListItem'
import type { NationProfile } from './bindings/NationProfile'
import type { NationOption } from './bindings/NationOption'
import type { PersonProfile } from './bindings/PersonProfile'
import type { SearchQuery } from './bindings/SearchQuery'
import type { SearchResult } from './bindings/SearchResult'

export const inTauri = isTauri()

export const api = {
  searchPeople: (query: SearchQuery) => invoke<SearchResult>('search_people', { query }),
  listNations: () => invoke<NationOption[]>('list_nations'),
  getPerson: (id: number) => invoke<PersonProfile>('get_person', { id }),
  getClub: (id: number) => invoke<ClubProfile>('get_club', { id }),
  listCompetitions: () => invoke<CompetitionListItem[]>('list_competitions'),
  getCompetition: (id: number) => invoke<CompetitionProfile>('get_competition', { id }),
  listNationsFull: () => invoke<NationListItem[]>('list_nations_full'),
  getNation: (id: number) => invoke<NationProfile>('get_nation', { id }),
}
