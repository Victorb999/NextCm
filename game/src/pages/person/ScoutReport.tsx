import type { PersonProfile } from '../../bindings/PersonProfile'
import { Empty, Panel } from '../../components/Screen'
import { cmDate } from '../../format'

/** Aba "Relatórios": o último relatório completo do olheiro (o "Report Card" do FM08). */
export default function ScoutReport({ p }: { p: PersonProfile }) {
  const card = p.scoutCard
  if (!card) return <Empty>Nenhum olheiro observou este jogador. Use Observar Jogador no menu Ação.</Empty>
  return (
    <div className="min-h-0 flex-1 overflow-auto pb-1">
      <Panel
        title="Relatório do Olheiro"
        right={
          <span className="font-sans text-[13px] text-cm-muted">
            {card.scout} · {cmDate(card.date)}
          </span>
        }
      >
        {card.sections.map((s) => (
          <div key={s.title} className="border-b border-cm-line last:border-b-0">
            <div className="flex h-7 items-center bg-cm-headdeep px-3">
              <span className="font-bold text-cm-mint">{s.title}</span>
              <span className="ml-auto font-bold text-cm-yellow">
                {s.stars != null ? (
                  <span title={`${s.stars} de 5`} className="tracking-[0.15em]">
                    {'★'.repeat(s.stars)}
                    <span className="text-white/25">{'★'.repeat(5 - s.stars)}</span>
                  </span>
                ) : (
                  s.value
                )}
              </span>
            </div>
            <div className="px-3 py-1.5 text-[14.5px] leading-relaxed text-cm-text">
              {s.lines.map((l) => (
                <div key={l}>{l}</div>
              ))}
            </div>
          </div>
        ))}
      </Panel>
    </div>
  )
}
