import { useSyncExternalStore } from 'react'

// O amistoso por marcar (Amistoso em Casa / Fora): a data escolhida espera que o treinador
// abra a página de um clube e carregue em "Convidar Clube", como no CM.

export type FriendlyPlan = { kind: 'home' | 'away'; date: string }

let plan: FriendlyPlan | null = null
const listeners = new Set<() => void>()

export function setFriendlyPlan(p: FriendlyPlan | null) {
  plan = p
  for (const l of listeners) l()
}

export function useFriendlyPlan(): FriendlyPlan | null {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l)
      return () => listeners.delete(l)
    },
    () => plan,
  )
}

/** A competição dos amigáveis de clubes no core. */
export const CLUB_FRIENDLY = 72
