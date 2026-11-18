import type { LiveView } from '../../bindings/LiveView'

type Kick = { side: number | null; kind: string; playerId: number | null }

/**
 * "Marcação de Pênaltis", como no fim do jogo do CM: as cobranças de cada lado, pela ordem,
 * "X marcou" em rosa e "X falhou" em verde. `kicks`: os lances `pen` / `penmiss` (um por cobrança).
 */
export function ShootoutList({ view, kicks, small }: { view: LiveView; kicks: Kick[]; small?: boolean }) {
  if (kicks.length === 0) return null
  const name = (side: number, id: number | null) => {
    const t = side === 0 ? view.home : view.away
    const p = [...t.players, ...t.out, ...t.bench].find((p) => p.id === id)
    return p ? p.listName.split(', ')[0] : '?'
  }
  return (
    <div className="flex w-full flex-col gap-2 px-2">
      <span
        className={`self-start border-b-2 border-cm-yellow font-title font-bold text-cm-yellow ${small ? 'text-[16px]' : 'text-[20px]'}`}
      >
        Marcação de Pênaltis
      </span>
      <div className="flex w-full gap-6">
        {[0, 1].map((side) => (
          <div key={side} className="min-w-0 flex-1">
            {kicks
              .filter((k) => k.side === side)
              .map((k, i) => (
                <div
                  key={i}
                  className={`truncate font-title ${small ? 'text-[15px] leading-5' : 'text-[20px] leading-7'} ${k.kind === 'pen' ? 'text-cm-magenta' : 'text-cm-mint'}`}
                >
                  {name(side, k.playerId)} {k.kind === 'pen' ? 'marcou' : 'falhou'}
                </div>
              ))}
          </div>
        ))}
      </div>
    </div>
  )
}
