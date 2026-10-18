import { invoke, isTauri } from '@tauri-apps/api/core'
import type { NationOption } from './bindings/NationOption'
import type { PersonProfile } from './bindings/PersonProfile'
import type { SearchQuery } from './bindings/SearchQuery'
import type { SearchResult } from './bindings/SearchResult'

export const inTauri = isTauri()

export const api = {
  searchPeople: (query: SearchQuery) => invoke<SearchResult>('search_people', { query }),
  listNations: () => invoke<NationOption[]>('list_nations'),
  getPerson: (id: number) => invoke<PersonProfile>('get_person', { id }),
}
