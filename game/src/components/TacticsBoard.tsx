import { useEffect, useState } from 'react'
import type { ClubTactics } from '../bindings/ClubTactics'
import type { Selection } from '../bindings/Selection'
import type { TacticPlayer } from '../bindings/TacticPlayer'
import { selectionOf, type Spot } from '../selection'

// Peças da tela de Tácticas do CM, usadas na tela de Tácticas e nas Tácticas a meio do jogo.

// Formações na ordem do core (FORMATIONS em core/src/tactics.rs); a 0 ("não definida") não aparece
export const FORMATIONS = ['4-4-2', '5-3-2 Líbero', '5-3-2', '4-4-2', '4-3-3', '4-2-4', '3-5-2', '4-1-2-1-2', '4-5-1', '3-4-3']
// Menu Tácticas, como no CM: Instruções à Equipa e as tácticas pré-definidas
export const TACTICS_MENU = [
  { key: 'instructions', label: 'Instruções à Equipa' },
  ...FORMATIONS.map((label, i) => ({ key: String(i), label: `Pré-definida: ${label}` })).slice(1),
]

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

export type Bind = { 'data-spot': string; onMouseDown: (e: React.MouseEvent) => void }

export function Row({
  p,
  kind,
  place,
  bind,
  onToggle,
  onOpen,
}: {
  p: TacticPlayer
  kind: 'starter' | 'bench' | 'reserve'
  /** Função (titular) ou SB1–SB7 (suplente). */
  place: string
  bind: Bind
  onToggle?: () => void
  /** Duplo clique (abre o perfil). */
  onOpen?: () => void
}) {
  const text = kind === 'bench' ? 'text-[#8a5ad8]' : kind === 'reserve' ? 'text-[#8a8a8a]' : 'text-white'
  const toggle =
    onToggle &&
    ((e: React.MouseEvent) => {
      e.stopPropagation()
      onToggle()
    })
  return (
    <div {...bind} onDoubleClick={onOpen} className="flex h-[19px] cursor-grab items-center gap-2 px-1 text-[13.5px]">
      {/* Número e casa: clicar põe/tira da equipa */}
      <span
        onMouseDown={(e) => onToggle && e.stopPropagation()}
        onClick={toggle}
        title={onToggle ? 'Clique para pôr/tirar da equipa' : undefined}
        className={`w-7 shrink-0 text-center tabular-nums cm-cell ${onToggle ? 'cursor-pointer' : ''} ${kind === 'bench' ? '!bg-[#1d2d8a] text-white' : 'text-white'}`}
      >
        {p.squadNumber ?? ''}
      </span>
      <span
        onMouseDown={(e) => onToggle && e.stopPropagation()}
        onClick={toggle}
        className={`w-9 shrink-0 text-center text-[11px] ${onToggle ? 'cursor-pointer' : ''} ${
          kind === 'starter' && outOfPosition(p) ? 'cm-cell !bg-[#7a3d00] text-cm-orange' : 'cm-cell text-cm-yellow'
        }`}
        title={kind === 'starter' && p.fit !== null ? `Nota na função: ${p.fit}` : undefined}
      >
        {place}
      </span>
      <span className={`flex-1 truncate ${text}`}>{fullName(p.listName)}</span>
      {(p.injured || p.suspended) && (
        <span className="shrink-0 bg-[#9c0000] px-1 text-[11px] font-bold text-white">{p.injured ? 'Les' : 'Sus'}</span>
      )}
      <span className={`w-24 shrink-0 truncate ${text}`}>{p.position}</span>
      <span className={`w-10 shrink-0 text-right ${conditionColour(p.condition)}`}>{p.condition ?? 100}%</span>
    </div>
  )
}

/** Seta tracejada (em % do campo), como as setas de movimento do CM. */
function Arrow({ from, to, live }: { from: [number, number]; to: [number, number]; live?: boolean }) {
  // Ponta da seta: dois traços curtos para trás a partir do alvo
  const [x1, y1] = from
  const [x2, y2] = to
  const a = Math.atan2(y2 - y1, x2 - x1)
  const head = (d: number) => [x2 - 3 * Math.cos(a + d), y2 - 3 * Math.sin(a + d)]
  const [hx1, hy1] = head(0.5)
  const [hx2, hy2] = head(-0.5)
  const colour = live ? '#ffff40' : '#ffffff'
  return (
    <g stroke={colour} strokeWidth={1.5} fill="none" vectorEffect="non-scaling-stroke">
      <line x1={x1} y1={y1} x2={x2} y2={y2} strokeDasharray="4 3" vectorEffect="non-scaling-stroke" />
      <polyline points={`${hx1},${hy1} ${x2},${y2} ${hx2},${hy2}`} vectorEffect="non-scaling-stroke" />
    </g>
  )
}

export type Run = { index: number; x: number; y: number }

/** Campo com a formação, como a "Vista Geral" do CM. Botão direito e arrastar a partir de
 * um jogador desenha a seta de movimento (para cima ataca, para baixo recua). */
export function Pitch({
  t,
  bind,
  run,
  onRunStart,
}: {
  t: ClubTactics
  bind: (s: Spot, label: string) => Bind
  /** Seta a ser desenhada agora (botão direito em curso). */
  run: Run | null
  onRunStart: (index: number) => void
}) {
  return (
    <div
      data-pitch
      onContextMenu={(e) => e.preventDefault()}
      className="relative min-h-0 flex-1 overflow-hidden border border-[#1d5a1d] bg-[repeating-linear-gradient(0deg,#1f6b22_0_36px,#1c6420_36px_72px)]"
    >
      {/* Linhas do campo */}
      <div className="pointer-events-none absolute inset-2 border border-white/40" />
      <div className="pointer-events-none absolute left-2 right-2 top-1/2 border-t border-white/40" />
      <div className="pointer-events-none absolute left-1/2 top-1/2 h-24 w-24 -translate-x-1/2 -translate-y-1/2 rounded-full border border-white/40" />
      <div className="pointer-events-none absolute left-1/2 top-2 h-[16%] w-[44%] -translate-x-1/2 border border-t-0 border-white/40" />
      <div className="pointer-events-none absolute bottom-2 left-1/2 h-[16%] w-[44%] -translate-x-1/2 border border-b-0 border-white/40" />
      <svg className="pointer-events-none absolute inset-0 h-full w-full" viewBox="0 0 100 100" preserveAspectRatio="none">
        {t.starters.map((s, i) => (s.run && run?.index !== i ? <Arrow key={i} from={[s.x, s.y]} to={[s.run[0], s.run[1]]} /> : null))}
        {run && <Arrow from={[t.starters[run.index].x, t.starters[run.index].y]} to={[run.x, run.y]} live />}
      </svg>
      {t.starters.map((s, i) => {
        const b = bind({ kind: 'starter', index: i }, s.player ? surname(s.player.listName) : s.role)
        return (
          <div
            key={i}
            {...b}
            onMouseDown={(e) => {
              if (e.button === 2 && t.editable && s.role !== 'GR') {
                e.preventDefault()
                onRunStart(i)
              } else b.onMouseDown(e)
            }}
            className={`absolute flex -translate-x-1/2 -translate-y-1/2 flex-col items-center ${t.editable ? 'cursor-grab' : ''}`}
            style={{ left: `${s.x}%`, top: `${s.y}%` }}
            title={s.role}
          >
            <span
              className={`flex h-6 w-6 items-center justify-center rounded-full border border-black text-[12px] font-bold ${
                s.role === 'GR' ? 'bg-[#0f9a2a] text-white' : s.player ? 'bg-white text-black' : 'bg-transparent text-white'
              }`}
            >
              {s.player?.squadNumber ?? ''}
            </span>
            <span
              className={`whitespace-nowrap text-[13px] font-bold [text-shadow:1px_1px_0_#000] ${
                outOfPosition(s.player) ? 'text-cm-orange' : s.player ? 'text-cm-yellow' : 'text-white/70'
              }`}
            >
              {s.player ? surname(s.player.listName) : shortRole(s.role)}
            </span>
          </div>
        )
      })}
    </div>
  )
}

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
