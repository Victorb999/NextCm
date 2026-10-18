import { useEffect, useRef, useState } from 'react'
import type { ClubTactics } from './bindings/ClubTactics'
import type { Selection } from './bindings/Selection'

// Escolha da equipa nas telas de Tácticas e Plantel: lugares, trocas e arrastar-e-soltar.

/** Lugar de um jogador na escolha: titular (função), suplente ou reserva. */
export type Spot = { kind: 'starter'; index: number } | { kind: 'bench'; index: number } | { kind: 'reserve'; id: number }

export const sameSpot = (a: Spot | null, b: Spot) =>
  !!a && a.kind === b.kind && (a.kind === 'reserve' ? a.id === (b as { id: number }).id : a.index === (b as { index: number }).index)

const encode = (s: Spot) => (s.kind === 'reserve' ? `r:${s.id}` : `${s.kind === 'starter' ? 's' : 'b'}:${s.index}`)

function decode(text: string | null | undefined): Spot | null {
  const [k, v] = (text ?? '').split(':')
  const n = Number(v)
  if (!Number.isInteger(n)) return null
  if (k === 's') return { kind: 'starter', index: n }
  if (k === 'b') return { kind: 'bench', index: n }
  if (k === 'r') return { kind: 'reserve', id: n }
  return null
}

export const selectionOf = (t: ClubTactics): Selection => ({
  formation: t.formationIndex,
  starters: t.starters.map((s) => s.player?.id ?? null),
  bench: t.bench.map((p) => p.id),
  instructions: t.instructions,
  auto: t.auto,
  // Posições livres só quando o treinador as mexeu (senão valem as da formação)
  positions: t.custom ? t.starters.map((s) => [s.x, s.y] as [number, number]) : [],
  runs: t.starters.map((s) => s.run),
})

/** Número de suplentes no banco (BENCH no core). */
export const BENCH = 9

/** Lugar atual de um jogador (titular, suplente ou reserva). */
export function spotOf(sel: Selection, id: number): Spot {
  const s = sel.starters.indexOf(id)
  if (s >= 0) return { kind: 'starter', index: s }
  const b = sel.bench.indexOf(id)
  if (b >= 0) return { kind: 'bench', index: b }
  return { kind: 'reserve', id }
}

function idAt(sel: Selection, spot: Spot): number | null {
  if (spot.kind === 'starter') return sel.starters[spot.index] ?? null
  if (spot.kind === 'bench') return sel.bench[spot.index] ?? null
  return spot.id
}

/** Troca dois lugares, como no CM. Quem sai do onze ou do banco vai para as reservas;
 * um lugar de suplente vazio (além do último) acrescenta o jogador ao banco. */
export function swap(sel: Selection, a: Spot, b: Spot): Selection {
  if (a.kind === 'reserve' && b.kind === 'reserve') return sel
  const next: Selection = { ...sel, starters: [...sel.starters], bench: [...sel.bench], auto: false }
  const ia = idAt(sel, a)
  const ib = idAt(sel, b)
  const put = (spot: Spot, id: number | null) => {
    if (spot.kind === 'starter') next.starters[spot.index] = id
    else if (spot.kind === 'bench') next.bench[spot.index] = id as number
  }
  put(a, ib)
  put(b, ia)
  // Buracos no banco (quem foi para as reservas) somem
  next.bench = next.bench.filter((id) => id !== null && id !== undefined)
  return next
}

/**
 * Arrastar-e-soltar para trocar jogadores (como no CM, trocar é só arrastando). `bind(spot, label)` dá as
 * props de cada linha/círculo (data-spot e onMouseDown); `ghost` é o nome que segue o rato.
 */
export function useDragSwap({
  enabled,
  onClick,
  onDrop,
  onDropPitch,
}: {
  enabled: boolean
  /** Clique sem arrastar (opcional). */
  onClick?: (spot: Spot) => void
  onDrop: (from: Spot, to: Spot) => void
  /** Soltar num lugar vazio do campo (elemento com `data-pitch`): x/y em % do campo. */
  onDropPitch?: (from: Spot, x: number, y: number) => void
}) {
  const [ghost, setGhost] = useState<{ label: string; x: number; y: number } | null>(null)
  const start = useRef<{ spot: Spot; label: string; x: number; y: number; moved: boolean } | null>(null)
  const handlers = useRef({ onClick, onDrop, onDropPitch })
  useEffect(() => {
    handlers.current = { onClick, onDrop, onDropPitch }
  })

  useEffect(() => {
    const move = (e: MouseEvent) => {
      const s = start.current
      if (!s) return
      if (!s.moved && Math.hypot(e.clientX - s.x, e.clientY - s.y) > 5) s.moved = true
      if (s.moved) setGhost({ label: s.label, x: e.clientX, y: e.clientY })
    }
    const up = (e: MouseEvent) => {
      const s = start.current
      start.current = null
      setGhost(null)
      if (!s) return
      if (!s.moved) return handlers.current.onClick?.(s.spot)
      const under = document.elementFromPoint(e.clientX, e.clientY)
      const to = decode(under?.closest('[data-spot]')?.getAttribute('data-spot'))
      if (to) {
        if (!sameSpot(s.spot, to)) handlers.current.onDrop(s.spot, to)
        return
      }
      const pitch = under?.closest('[data-pitch]')
      if (pitch && handlers.current.onDropPitch) {
        const r = pitch.getBoundingClientRect()
        handlers.current.onDropPitch(s.spot, ((e.clientX - r.left) / r.width) * 100, ((e.clientY - r.top) / r.height) * 100)
      }
    }
    window.addEventListener('mousemove', move)
    window.addEventListener('mouseup', up)
    return () => {
      window.removeEventListener('mousemove', move)
      window.removeEventListener('mouseup', up)
    }
  }, [])

  const bind = (spot: Spot, label: string) => ({
    'data-spot': encode(spot),
    onMouseDown: (e: React.MouseEvent) => {
      if (!enabled || e.button !== 0) return
      e.preventDefault()
      start.current = { spot, label, x: e.clientX, y: e.clientY, moved: false }
    },
  })

  const ghostEl = ghost && (
    <div
      className="pointer-events-none fixed z-50 -translate-x-1/2 -translate-y-1/2 border border-cm-yellow bg-cm-purple px-2 py-0.5 font-bold text-cm-yellow shadow-[2px_2px_0_rgba(0,0,0,0.6)]"
      style={{ left: ghost.x, top: ghost.y }}
    >
      {ghost.label}
    </div>
  )

  return { bind, ghost: ghostEl, dragging: !!ghost }
}
