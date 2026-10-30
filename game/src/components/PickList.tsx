import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { Empty } from './Screen'

export type PickItem = { id: number; label: string; detail?: ReactNode }
export type PickGroup = { title?: ReactNode; items: PickItem[] }

/**
 * Lista em 2 colunas (nome e o detalhe numa etiqueta amarela), com grupos: nações, clubes, competições.
 * Clique seleciona, duplo clique/Enter abre; setas navegam (↑↓ pulam 2, ←→ 1).
 */
export default function PickList({
  groups,
  onOpen,
  onSelect,
  empty = 'Nada para mostrar.',
}: {
  groups: PickGroup[]
  onOpen: (item: PickItem) => void
  /** Avisado quando a seleção muda (clique ou setas). */
  onSelect?: (item: PickItem) => void
  empty?: string
}) {
  const [selected, setSelected] = useState(0)
  const ref = useRef<HTMLDivElement>(null)
  const flat = groups.flatMap((g) => g.items)
  const current = flat[selected]

  useEffect(() => {
    if (current) onSelect?.(current)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [current?.id])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey) return
      const step = { ArrowUp: -2, ArrowDown: 2, ArrowLeft: -1, ArrowRight: 1 }[e.key]
      if (step !== undefined) {
        e.preventDefault()
        setSelected((s) => {
          const next = Math.max(0, Math.min(flat.length - 1, s + step))
          ref.current?.querySelector(`[data-row="${next}"]`)?.scrollIntoView({ block: 'nearest' })
          return next
        })
      } else if (e.key === 'Enter' && flat[selected]) {
        onOpen(flat[selected])
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  if (flat.length === 0) return <Empty>{empty}</Empty>

  let index = 0
  return (
    <div ref={ref} className="min-h-0 flex-1 overflow-auto px-2 pb-2">
      {groups.map((g, gi) =>
        g.items.length === 0 ? null : (
          <div key={gi}>
            {g.title && (
              <div className="pb-1 pt-3 font-mono text-[12px] font-bold uppercase tracking-[0.12em] text-cm-orange">{g.title}</div>
            )}
            <div className="cm-zebra-2 grid grid-cols-2 gap-x-3 overflow-hidden rounded">
              {g.items.map((item) => {
                const i = index++
                return (
                  <div
                    key={`${gi}-${item.id}`}
                    data-row={i}
                    onMouseDown={() => setSelected(i)}
                    onDoubleClick={() => onOpen(item)}
                    className={`flex h-7 cursor-pointer items-center gap-2 px-3 ${i === selected ? '!bg-cm-sel' : ''}`}
                  >
                    <span className="flex-1 truncate text-[15px] font-semibold text-white">{item.label}</span>
                    {item.detail !== undefined && item.detail !== '' && (
                      <span
                        className={`shrink-0 truncate rounded border px-1.5 font-mono text-[11px] font-bold ${
                          i === selected ? 'border-white/60 text-white' : 'border-cm-yellow/40 text-cm-yellow'
                        }`}
                      >
                        {item.detail}
                      </span>
                    )}
                  </div>
                )
              })}
            </div>
          </div>
        ),
      )}
    </div>
  )
}
