import { useEffect } from 'react'
import type { DependencyList } from 'react'

/**
 * Carrega dados do core quando `deps` mudam e entrega-os a `onData` (ou o erro a `onError`).
 * A resposta de um pedido já ultrapassado (o jogador ou o clube mudou entretanto) é ignorada,
 * para uma resposta antiga não apagar a nova. O estado continua no componente.
 */
export function useLoad<T>(load: () => Promise<T>, deps: DependencyList, onData: (v: T) => void, onError: (e: string) => void) {
  useEffect(() => {
    let current = true
    load().then(
      (v) => current && onData(v),
      (e) => current && onError(String(e)),
    )
    return () => {
      current = false
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps)
}
