import type { PersonProfile } from '../bindings/PersonProfile'
import { cmDate } from '../format'
import { Empty, Panel } from './Screen'

/** A aba Marcos & Recordes do perfil: os recordes do jogador e os marcos da carreira (do core). */
export default function PersonRecords({ p }: { p: PersonProfile }) {
  if (!p.records.length && !p.marks.length) return <Empty>Sem registos.</Empty>
  return (
    <div className="flex min-h-0 flex-1 items-start gap-2 overflow-auto pb-1">
      <Panel title="Recordes" className="w-[42%] shrink-0">
        <div className="cm-zebra">
          {p.records.map((r) => (
            <div key={r.label} className="px-3 py-0.5 text-[13.5px]">
              <div className="flex min-h-6 items-center gap-2">
                <span className="min-w-0 flex-1 truncate text-cm-muted">{r.label}</span>
                <span className="shrink-0 text-right font-bold text-cm-yellow">{r.value}</span>
              </div>
              {r.detail && <div className="-mt-1 truncate pb-0.5 text-[12px] text-[#9a9aa6]">{r.detail}</div>}
            </div>
          ))}
        </div>
      </Panel>
      <Panel title="Marcos" chip={p.marks.length ? `${p.marks.length}` : undefined} className="min-w-0 flex-1">
        {p.marks.length ? (
          <div className="cm-zebra">
            {p.marks.map((m, i) => (
              <div key={`${m.date}-${i}`} className="flex min-h-6 items-center gap-3 px-3 text-[13.5px]">
                <span className="w-20 shrink-0 font-mono text-[12.5px] text-cm-yellow">{cmDate(m.date)}</span>
                <span className="min-w-0 flex-1 truncate text-white">{m.text}</span>
              </div>
            ))}
          </div>
        ) : (
          <Empty>Ainda sem marcos neste jogo.</Empty>
        )}
      </Panel>
    </div>
  )
}
