import { useState } from 'react'
import type { LivePlayer } from '../../bindings/LivePlayer'
import { PanelTitle } from '../Screen'

/**
 * A ordem dos batedores antes da disputa de pênaltis (nas Táticas, como no CM): clicar num
 * batedor e depois noutro troca os dois de lugar. Batem pela ordem e, se todos baterem,
 * volta ao primeiro.
 */
export function ShootoutTakers({
  players,
  order,
  onChange,
}: {
  /** Quem está em campo. */
  players: LivePlayer[]
  order: number[]
  onChange: (order: number[]) => void
}) {
  const [picked, setPicked] = useState<number | null>(null)
  const name = (id: number) =>
    players
      .find((p) => p.id === id)
      ?.listName.split(', ')
      .reverse()
      .join(' ') ?? '?'
  const position = (id: number) => players.find((p) => p.id === id)?.position ?? ''
  const click = (i: number) => {
    if (picked === null) return setPicked(i)
    if (picked !== i) {
      onChange(order.map((id, k) => (k === picked ? order[i]! : k === i ? order[picked]! : id)))
    }
    setPicked(null)
  }
  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-md bg-cm-surface">
      <PanelTitle title="Batedores de Pênaltis" chip={`${order.length}`} />
      <div className="cm-zebra min-h-0 flex-1 overflow-y-auto">
        {order.map((id, i) => (
          <div
            key={id}
            onClick={() => click(i)}
            className={`flex h-[30px] cursor-pointer items-center gap-2 px-3 text-[14px] hover:bg-cm-hover ${
              picked === i ? 'bg-cm-hover text-cm-yellow ring-1 ring-inset ring-cm-yellow' : 'text-white'
            }`}
          >
            <span className="w-8 shrink-0 text-center font-mono font-bold text-cm-orange">{i + 1}º</span>
            <span className="min-w-0 flex-1 truncate">{name(id)}</span>
            <span className="shrink-0 text-[12px] text-cm-muted">{position(id)}</span>
          </div>
        ))}
      </div>
      <div className="shrink-0 px-2 py-1 text-center text-[12px] text-[#aab3c4]">
        {picked === null
          ? 'Clique num batedor e depois noutro para trocar a ordem · Ok começa a disputa'
          : `Trocar ${name(order[picked] ?? 0)} com...`}
      </div>
    </div>
  )
}
