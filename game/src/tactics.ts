import { useEffect, useState } from 'react'
import type { ClubTactics } from './bindings/ClubTactics'
import type { Mentality } from './bindings/Mentality'
import type { Passing } from './bindings/Passing'
import type { PlayerOrders } from './bindings/PlayerOrders'
import type { Tackling } from './bindings/Tackling'
import type { Selection } from './bindings/Selection'
import type { TacticPlayer } from './bindings/TacticPlayer'
import { selectionOf } from './selection'

// Funções da tela de Tácticas (nomes, formações, posições livres e a seta de movimento),
// partilhadas pela tela de Tácticas e pelas Tácticas a meio do jogo.

// Formações na ordem do core (FORMATIONS em core/src/tactics.rs); a 0 ("não definida") não aparece
export const FORMATIONS = ['4-4-2', '5-3-2 Líbero', '5-3-2', '4-4-2', '4-3-3', '4-2-4', '3-5-2', '4-1-2-1-2', '4-5-1', '3-4-3']
// Menu Tácticas, como no CM: Instruções à Equipa e as tácticas pré-definidas
export const TACTICS_MENU = [
  { key: 'instructions', label: 'Instruções à Equipa' },
  ...FORMATIONS.map((label, i) => ({ key: String(i), label: `Pré-definida: ${label}` })).slice(1),
]

// Opções das Instruções à Equipa (diálogo e resumo da tela de Tácticas)
export const MENTALITY: [Mentality, string][] = [
  ['normal', 'Normal'],
  ['defensive', 'Defensivo'],
  ['attacking', 'Atacante'],
]
export const PASSING: [Passing, string][] = [
  ['mixed', 'Misto'],
  ['short', 'Curto'],
  ['direct', 'Directo'],
  ['long', 'Longo'],
]
export const TACKLING: [Tackling, string][] = [
  ['normal', 'Normal'],
  ['soft', 'Suave'],
  ['hard', 'Duro'],
]

/** Instruções individuais iguais às da equipa. */
export const NO_ORDERS: PlayerOrders = {
  passing: null,
  tackling: null,
  pressing: null,
  passTo: 'mixed',
  setAttack: 'normal',
  setDefence: 'normal',
  freeRole: false,
  forwardRuns: false,
  runWithBall: false,
  holdBall: false,
  longShots: false,
  throughBalls: false,
  crosses: false,
  marking: 'zonal',
}

/** As instruções individuais de um jogador da escolha. */
export const ordersOf = (t: ClubTactics, id: number) => t.orders.find((o) => o.personId === id)?.orders ?? NO_ORDERS

/** A escolha com as instruções individuais de um jogador mudadas. */
export function withOrders(sel: Selection, id: number, orders: PlayerOrders): Selection {
  return { ...sel, orders: [...sel.orders.filter((o) => o.personId !== id), { personId: id, orders }] }
}

/** "Sobrenome, Nome" -> "Sobrenome" (como os nomes no campo do CM). */
export const surname = (listName: string) => listName.split(', ')[0]

/** Nome na lista: "Nome Sobrenome", como na tela de tácticas do CM. */
export const fullName = (listName: string) => {
  const [last, first] = listName.split(', ')
  return first ? `${first} ${last}` : last
}

/** "D D" -> "DD": a sigla curta da função. */
export const shortRole = (role: string) => role.replace(' ', '')

/** Fora da posição: nota na função abaixo de 15 (mostrada em laranja). */
export const outOfPosition = (p: TacticPlayer | null | undefined) => !!p && p.fit !== null && p.fit < 15

export function conditionColour(c: number | null) {
  if (c === null || c >= 90) return 'text-cm-yellow'
  if (c >= 75) return 'text-cm-orange'
  return 'text-[#ff4040]'
}

/** Seta a ser desenhada agora (botão direito em curso), em % do campo. */
export type Run = { index: number; x: number; y: number }

/** A seta de movimento com o botão direito: segue o rato e, ao soltar, entrega a escolha
 * com a seta nova (soltar em cima do próprio jogador apaga-a). */
export function useRunArrow(t: ClubTactics | null, apply: (sel: Selection) => void) {
  const [run, setRun] = useState<Run | null>(null)
  useEffect(() => {
    if (!run) return
    const pitch = document.querySelector('[data-pitch]')
    if (!pitch) return
    const at = (e: MouseEvent) => {
      const r = pitch.getBoundingClientRect()
      const clamp = (v: number) => Math.round(Math.min(98, Math.max(2, v)))
      return [clamp(((e.clientX - r.left) / r.width) * 100), clamp(((e.clientY - r.top) / r.height) * 100)]
    }
    const move = (e: MouseEvent) => {
      const [x, y] = at(e)
      setRun((cur) => cur && { ...cur, x, y })
    }
    const up = (e: MouseEvent) => {
      const [x, y] = at(e)
      const index = run.index
      setRun(null)
      if (!t) return
      const slot = t.starters[index]
      const runs = t.starters.map((s) => s.run)
      runs[index] = Math.hypot(x - slot.x, y - slot.y) < 5 ? null : [x, y]
      apply({ ...selectionOf(t), runs, auto: false })
    }
    window.addEventListener('mousemove', move)
    window.addEventListener('mouseup', up)
    return () => {
      window.removeEventListener('mousemove', move)
      window.removeEventListener('mouseup', up)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [run?.index])
  const start = (index: number) => t && setRun({ index, x: t.starters[index].x, y: t.starters[index].y })
  return { run, start }
}

/** Soltar um titular num lugar vazio do campo: a escolha com a posição (e a função) nova. */
export function moveOnPitch(t: ClubTactics, index: number, x: number, y: number): Selection | null {
  if (t.starters[index].role === 'GR') return null
  const positions = t.starters.map((s) => [s.x, s.y] as [number, number])
  positions[index] = [Math.round(Math.min(96, Math.max(4, x))), Math.round(Math.min(85, Math.max(4, y)))]
  return { ...selectionOf(t), positions, auto: false }
}
