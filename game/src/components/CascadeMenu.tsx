import { useEffect, useLayoutEffect, useRef, useState } from 'react'

// Menu em cascata, como o do botão "Competições" do CM 01/02, no verde dos painéis novos e com zebra.

export type MenuNode = { label: string; onSelect?: () => void; children?: MenuNode[] } | 'separator'

const ROW = 'flex h-6 w-full items-center justify-between gap-4 whitespace-nowrap px-3 text-left'

function Panel({
  items,
  x,
  y,
  back,
  onClose,
}: {
  items: MenuNode[]
  x: number
  y: number
  /** Num submenu, onde acaba o menu de cima pela esquerda: sem espaço à direita, abre para lá. */
  back?: number
  onClose: () => void
}) {
  const ref = useRef<HTMLDivElement>(null)
  const [top, setTop] = useState(y)
  const [left, setLeft] = useState(x)
  const [open, setOpen] = useState<{ index: number; x: number; y: number; back: number } | null>(null)

  // Sobe o painel para caber na janela (o CM faz o mesmo nos menus longos) e, sem espaço à
  // direita, abre-o para a esquerda (um menu do canto direito do ecrã)
  useLayoutEffect(() => {
    const h = ref.current?.offsetHeight ?? 0
    const w = ref.current?.offsetWidth ?? 0
    setTop(Math.max(0, Math.min(y, window.innerHeight - h)))
    const room = window.innerWidth - 4
    setLeft(x + w <= room ? x : Math.max(0, back !== undefined ? back - w + 2 : room - w))
  }, [x, y, back, items])

  const sub = open && items[open.index]
  return (
    <>
      <div
        ref={ref}
        style={{ left, top }}
        className="cm-zebra fixed z-[70] max-h-screen min-w-[170px] overflow-y-auto rounded-md border border-cm-dlgborder/60 bg-cm-dlg py-1 text-[14px] text-cm-text shadow-[0_8px_24px_rgba(0,0,0,0.65)]"
      >
        {items.map((item, i) =>
          item === 'separator' ? (
            <div key={i} className="mx-2 my-1 border-t border-cm-dlgline" />
          ) : (
            <button
              key={i}
              className={`${ROW} ${open?.index === i ? '!bg-cm-dlghover text-cm-yellow' : 'hover:!bg-cm-dlghover hover:text-cm-yellow'}`}
              onMouseEnter={(e) => {
                const r = e.currentTarget.getBoundingClientRect()
                setOpen(item.children ? { index: i, x: r.right - 2, y: r.top - 2, back: r.left } : null)
              }}
              onClick={() => {
                if (item.onSelect) {
                  item.onSelect()
                  onClose()
                }
              }}
            >
              <span>{item.label}</span>
              {item.children && <span className="text-[10px] text-cm-mint">▶</span>}
            </button>
          ),
        )}
      </div>
      {sub && sub !== 'separator' && sub.children && (
        <Panel key={open.index} items={sub.children} x={open.x} y={open.y} back={open.back} onClose={onClose} />
      )}
    </>
  )
}

/** Abre o menu na posição dada; fecha com clique fora ou Esc. */
export default function CascadeMenu({
  items,
  x,
  y,
  onClose,
}: {
  items: MenuNode[]
  x: number
  y: number
  onClose: () => void
}) {
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      // O botão que abriu o menu cuida de fechá-lo (senão fecharia e reabriria)
      const t = e.target as Element
      if (!ref.current?.contains(t) && !t.closest('[data-menu-anchor]')) onClose()
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopImmediatePropagation()
        onClose()
      }
    }
    window.addEventListener('mousedown', onDown)
    window.addEventListener('keydown', onKey, true)
    return () => {
      window.removeEventListener('mousedown', onDown)
      window.removeEventListener('keydown', onKey, true)
    }
  }, [onClose])
  return (
    <div ref={ref}>
      <Panel items={items} x={x} y={y} onClose={onClose} />
    </div>
  )
}
