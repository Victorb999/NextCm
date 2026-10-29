import { useEffect, useId, useSyncExternalStore } from 'react'

// Os botões das telas (Atrás / Seguinte, Cancelar / Ok) vão para a barra de cima: só os da
// tela aberta por último aparecem (a Pesquisa fica montada escondida; a palestra e as tácticas
// a meio do jogo abrem por cima de outra tela).

let stack: string[] = []
const listeners = new Set<() => void>()

const subscribe = (f: () => void) => {
  listeners.add(f)
  return () => listeners.delete(f)
}
const changed = () => listeners.forEach((f) => f())

/** Regista a barra enquanto `on`; devolve se é ela que aparece. */
export function useTopActions(on: boolean): boolean {
  const id = useId()
  useEffect(() => {
    if (!on) return
    stack = [...stack, id]
    changed()
    return () => {
      stack = stack.filter((x) => x !== id)
      changed()
    }
  }, [id, on])
  return useSyncExternalStore(subscribe, () => stack[stack.length - 1] === id)
}
