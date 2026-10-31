import { useEffect, useState } from 'react'
import type { ClubTactics } from './bindings/ClubTactics'
import type { Marking } from './bindings/Marking'
import type { Mentality } from './bindings/Mentality'
import type { PassTo } from './bindings/PassTo'
import type { Passing } from './bindings/Passing'
import type { PlayerOrders } from './bindings/PlayerOrders'
import type { SetPiece } from './bindings/SetPiece'
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
  { key: 'load', label: 'Carregar Táctica' },
  { key: 'save', label: 'Gravar Táctica' },
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

// Opções das instruções individuais (diálogo do jogador e o Vêr das Tácticas)
export const ORDER_PASSING: [Passing | null, string][] = [
  [null, 'Equipa'],
  ['mixed', 'Misto'],
  ['short', 'Curto'],
  ['direct', 'Directo'],
  ['long', 'Longo'],
]
export const ORDER_TACKLING: [Tackling | null, string][] = [
  [null, 'Equipa'],
  ['normal', 'Normal'],
  ['soft', 'Suave'],
  ['hard', 'Duro'],
]
export const ORDER_PRESSING: [boolean | null, string][] = [
  [null, 'Equipa'],
  [false, 'Não'],
  [true, 'Sim'],
]
export const ORDER_PASS_TO: [PassTo, string][] = [
  ['mixed', 'D/E/C'],
  ['left', 'Esquerda'],
  ['centre', 'Centro'],
  ['right', 'Direita'],
]
export const ORDER_SET_PIECE: [SetPiece, string][] = [
  ['normal', 'Normal'],
  ['back', 'Atrás'],
  ['forward', 'Frente'],
]
export const ORDER_MARKING: [Marking, string][] = [
  ['zonal', 'Zona'],
  ['none', 'Nenhuma'],
  ['manToMan', 'H-H'],
]
export const ORDER_FLAGS: [keyof PlayerOrders, string][] = [
  ['freeRole', 'Acção Livre'],
  ['forwardRuns', 'Movimentos Frente'],
  ['runWithBall', 'Correr com Bola'],
  ['holdBall', 'Segurar Bola'],
  ['longShots', 'Remates Longe'],
  ['throughBalls', 'Passes Costas'],
  ['crosses', 'Cruzamentos'],
]

/** O Vêr das Tácticas, como no CM: o que mostra a coluna da direita da lista. */
export type TacticsView =
  | 'position'
  | 'condition'
  | 'goals'
  | 'form'
  | 'rating'
  | 'passing'
  | 'tackling'
  | 'pressing'
  | 'passTo'
  | 'setAttack'
  | 'setDefence'
  | 'freeRole'
  | 'forwardRuns'
  | 'runWithBall'
  | 'holdBall'
  | 'longShots'
  | 'throughBalls'
  | 'crosses'
  | 'marking'

export const TACTICS_VIEWS: ({ key: TacticsView; label: string } | 'separator')[] = [
  { key: 'position', label: 'Posição' },
  { key: 'condition', label: 'Condição' },
  { key: 'goals', label: 'Golos' },
  { key: 'form', label: 'Forma' },
  { key: 'rating', label: 'Classificação Média' },
  'separator',
  { key: 'passing', label: 'Passe' },
  { key: 'tackling', label: 'Desarme' },
  { key: 'pressing', label: 'Pressão' },
  { key: 'passTo', label: 'Passar para' },
  { key: 'setAttack', label: 'Bolas Paradas (A)' },
  { key: 'setDefence', label: 'Bolas Paradas (D)' },
  { key: 'freeRole', label: 'Acção Livre' },
  { key: 'forwardRuns', label: 'Movimentos Frente' },
  { key: 'runWithBall', label: 'Correr com Bola' },
  { key: 'holdBall', label: 'Segurar Bola' },
  { key: 'longShots', label: 'Remates Longe' },
  { key: 'throughBalls', label: 'Passes Costas Defesa' },
  { key: 'crosses', label: 'Cruzamentos' },
  { key: 'marking', label: 'Marcação' },
]

const pick = <T,>(options: [T, string][], v: T) => options.find(([k]) => k === v)?.[1] ?? '-'

/** O valor da coluna do Vêr para um jogador; `on` pinta-o a amarelo (os números, e nas instruções o Sim
 * ou o que não é o da equipa). */
export function viewValue(view: TacticsView, p: TacticPlayer, o: PlayerOrders): { text: string; on: boolean } {
  switch (view) {
    case 'position':
      return { text: p.position, on: false }
    case 'condition':
      return { text: `${p.condition ?? 100}%`, on: true }
    case 'goals':
      return { text: String(p.goals), on: true }
    case 'form':
      return { text: p.form.length ? p.form.map((r) => Math.round(r)).join('-') : '-', on: true }
    case 'rating':
      return { text: p.rating === null ? '-' : p.rating.toFixed(2).replace('.', ','), on: true }
    case 'passing':
      return { text: pick(ORDER_PASSING, o.passing), on: o.passing !== null }
    case 'tackling':
      return { text: pick(ORDER_TACKLING, o.tackling), on: o.tackling !== null }
    case 'pressing':
      return { text: pick(ORDER_PRESSING, o.pressing), on: o.pressing !== null }
    case 'passTo':
      return { text: pick(ORDER_PASS_TO, o.passTo), on: o.passTo !== 'mixed' }
    case 'setAttack':
      return { text: pick(ORDER_SET_PIECE, o.setAttack), on: o.setAttack !== 'normal' }
    case 'setDefence':
      return { text: pick(ORDER_SET_PIECE, o.setDefence), on: o.setDefence !== 'normal' }
    case 'marking':
      return { text: pick(ORDER_MARKING, o.marking), on: o.marking !== 'zonal' }
    default:
      return { text: o[view] ? 'Sim' : 'Não', on: !!o[view] }
  }
}

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

/** Cor da condição: verde em forma, amarelo a cansar, vermelho esgotado. */
export function conditionColour(c: number | null) {
  if (c === null || c >= 90) return 'text-[#4edea3]'
  if (c >= 75) return 'text-cm-yellow'
  return 'text-[#ff6060]'
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

/** O rótulo do Vêr escolhido (o título da coluna). */
export const viewLabel = (view: TacticsView) =>
  TACTICS_VIEWS.find((v): v is { key: TacticsView; label: string } => v !== 'separator' && v.key === view)?.label ?? 'Posição'
