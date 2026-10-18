import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'

export type PickItem = { id: number; label: string; detail?: ReactNode }
export type PickGroup = { title?: ReactNode; items: PickItem[] }

/**
 * Lista em 2 colunas no estilo do CM (nomes grandes, detalhe em amarelo), com grupos.
 * Clique seleciona, duplo clique/Enter abre; setas navegam (↑↓ pulam 2, ←→ 1).
 */
export default function PickList({
  groups,
  onOpen,
  empty = 'Nada para mostrar.',
}: {
  groups: PickGroup[]
  onOpen: (item: PickItem) => void
  empty?: string
}) {
  const [selected, setSelected] = useState(0)
  const ref = useRef<HTMLDivElement>(null)
  const flat = groups.flatMap((g) => g.items)

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

  if (flat.length === 0) return <div className="p-6 text-center text-cm-magenta">{empty}</div>

  let index = 0
  return (
    <div ref={ref} className="min-h-0 flex-1 overflow-auto px-2 pb-2">
      {groups.map((g, gi) =>
        g.items.length === 0 ? null : (
          <div key={gi}>
            {g.title && (
              <div className="pb-1 pt-2 text-center font-title text-lg font-bold text-cm-yellow">{g.title}</div>
            )}
            <div className="grid grid-cols-2 gap-x-3">
              {g.items.map((item) => {
                const i = index++
                return (
                  <div
                    key={`${gi}-${item.id}`}
                    data-row={i}
                    onMouseDown={() => setSelected(i)}
                    onDoubleClick={() => onOpen(item)}
                    className={`flex h-[26px] items-center gap-2 px-2 ${i === selected ? 'bg-cm-sel' : 'bg-black/40'}`}
                  >
                    <span className="flex-1 truncate font-title text-[17px] text-white">{item.label}</span>
                    {item.detail !== undefined && (
                      <span className={`shrink-0 truncate ${i === selected ? 'text-white' : 'text-cm-yellow'}`}>
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
