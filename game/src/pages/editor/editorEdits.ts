// As mudanças do Editor de Jogos Salvos por gravar: cada jogador, clube e membro do staff aberto
// fica aqui com o que veio do save e o rascunho. "Gravar Alterações" junta tudo num EditorChanges
// e o core grava de uma vez (tudo ou nada).

import type { EditorChanges } from '../../bindings/EditorChanges'
import type { EditorClub } from '../../bindings/EditorClub'
import type { EditorFilter } from '../../bindings/EditorFilter'
import type { EditorPlayer } from '../../bindings/EditorPlayer'
import type { EditorProfile } from '../../bindings/EditorProfile'
import type { EditorProfileView } from '../../bindings/EditorProfileView'
import type { EditorStaff } from '../../bindings/EditorStaff'
import type { EditorStaffData } from '../../bindings/EditorStaffData'

export type PlayerEdit = {
  player: EditorPlayer
  view: EditorProfileView
  /** Atributos mudados (valor mostrado, 1 a 20). */
  draft: Record<string, number>
  profile: EditorProfile
}
export type ClubEdit = { saved: EditorClub; draft: EditorClub }
export type StaffEdit = { staff: EditorStaff; data: EditorStaffData; draft: Record<string, number> }
export type Edits = {
  players: Record<number, PlayerEdit>
  clubs: Record<number, ClubEdit>
  staff: Record<number, StaffEdit>
}

export const NO_EDITS: Edits = { players: {}, clubs: {}, staff: {} }

export const EMPTY_FILTER: EditorFilter = {
  query: '',
  minAge: null,
  maxAge: null,
  minAbility: null,
  maxAbility: null,
  minPotential: null,
  maxPotential: null,
  minReputation: null,
  maxReputation: null,
  position: null,
  nationId: null,
  basedId: null,
  divisionId: null,
}

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b)

/** Os atributos que mudaram (valor diferente do mostrado). */
export function changedValues(attrs: { key: string; shown: number }[], draft: Record<string, number>) {
  return attrs.filter((a) => draft[a.key] !== undefined && draft[a.key] !== a.shown).map((a) => ({ key: a.key, value: draft[a.key]! }))
}

export const playerDirty = (e: PlayerEdit) => !same(e.view.profile, e.profile) || changedValues(e.player.attributes, e.draft).length > 0
export const clubDirty = (e: ClubEdit) => !same(e.saved, e.draft)
export const staffDirty = (e: StaffEdit) => !same(e.staff.data, e.data) || changedValues(e.staff.attributes, e.draft).length > 0

/** Os nomes de quem tem mudanças por gravar. */
export function dirtyNames(edits: Edits): string[] {
  return [
    ...Object.values(edits.players)
      .filter(playerDirty)
      .map((e) => e.player.name),
    ...Object.values(edits.clubs)
      .filter(clubDirty)
      .map((e) => e.saved.name),
    ...Object.values(edits.staff)
      .filter(staffDirty)
      .map((e) => e.staff.name),
  ]
}

/** Junta as mudanças para o `editor_save`. */
export function collectChanges(edits: Edits): EditorChanges {
  return {
    players: Object.values(edits.players)
      .filter(playerDirty)
      .map((e) => ({
        id: e.player.id,
        name: e.player.name,
        profile: same(e.view.profile, e.profile) ? null : e.profile,
        values: changedValues(e.player.attributes, e.draft),
      })),
    staff: Object.values(edits.staff)
      .filter(staffDirty)
      .map((e) => ({ id: e.staff.id, name: e.staff.name, data: e.data, values: changedValues(e.staff.attributes, e.draft) })),
    clubs: Object.values(edits.clubs)
      .filter(clubDirty)
      .map((e) => e.draft),
  }
}

/** Desfaz todas as mudanças (fica o que veio do save, os abertos continuam abertos). */
export function revertAll(edits: Edits): Edits {
  const map = <T>(o: Record<number, T>, f: (v: T) => T) => Object.fromEntries(Object.entries(o).map(([k, v]) => [k, f(v)]))
  return {
    players: map(edits.players, (e) => ({ ...e, draft: {}, profile: e.view.profile })),
    clubs: map(edits.clubs, (e) => ({ ...e, draft: e.saved })),
    staff: map(edits.staff, (e) => ({ ...e, data: e.staff.data, draft: {} })),
  }
}
