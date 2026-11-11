import type { LastLineup } from '../bindings/LastLineup'
import { cmDate } from '../format'
import { fullName, shortRole } from '../tactics'
import { BottomBar, PanelTitle } from './Screen'
import { Pitch } from './TacticsBoard'

// "Último Jogo" das Tácticas, como no CM: a escalação do último jogo do clube ("Formação vs
// Celta"), o onze e o banco na lista e as posições no campo. Só para ver.

const noBind = () => ({ 'data-spot': '', onMouseDown: () => {} })

export default function LastLineupView({ last, colours, onBack }: { last: LastLineup; colours: { fg: string; bg: string }; onBack: () => void }) {
  const t = { ...last.tactics, editable: false }
  return (
    <div className="absolute inset-0 z-20 flex flex-col bg-cm-screen">
      <div className="mx-2 mt-2 flex min-h-0 flex-1 gap-2">
        <div className="flex min-h-0 w-[46%] flex-col overflow-hidden rounded-md bg-cm-surface">
          <PanelTitle title={`Formação vs ${last.opponent.shortName}`} chip={cmDate(last.date)} />
          <div className="cm-zebra min-h-0 flex-1 overflow-auto">
            {t.starters.map((s, i) => (
              <div key={`s${i}`} className="flex h-[28px] items-center gap-2 px-3 text-[14.5px]">
                <span className="w-8 shrink-0 text-center font-semibold tabular-nums text-white">{s.player?.squadNumber ?? ''}</span>
                <span className="min-w-0 flex-1 truncate text-white">{s.player ? fullName(s.player.listName) : '—'}</span>
                <span className="w-16 shrink-0 text-right text-[13px] text-cm-yellow">{shortRole(s.role)}</span>
              </div>
            ))}
            {t.bench.map((p) => (
              <div key={`b${p.id}`} className="flex h-[28px] items-center gap-2 px-3 text-[14.5px]">
                <span className="w-8 shrink-0 text-center font-semibold tabular-nums text-cm-purplelight">{p.squadNumber ?? ''}</span>
                <span className="min-w-0 flex-1 truncate text-cm-purplelight">{fullName(p.listName)}</span>
                <span className="w-16 shrink-0 text-right text-[13px] text-cm-purplelight">{p.position}</span>
              </div>
            ))}
          </div>
        </div>
        <div className="flex min-h-0 flex-1 flex-col">
          <Pitch t={t} colours={colours} bind={noBind} run={null} onRunStart={() => {}} />
        </div>
      </div>
      <BottomBar onLeft={onBack} />
    </div>
  )
}
