import type { LeagueChoice } from '../bindings/LeagueChoice'
import type { LeagueMode } from '../bindings/LeagueMode'
import type { LeagueNation } from '../bindings/LeagueNation'
import { DialogFrame, Overlay } from '../components/Dialog'

// "Escolher Campeonato(s)", como no CM: cada nação Escolhida (joga-se com tudo e pode-se
// treinar lá), de Fundo (só os resultados) ou fora do jogo.

const MODES: [LeagueMode, string][] = [
  ['play', 'ESCOLHIDO'],
  ['background', 'FUNDO'],
  ['off', 'FORA'],
]

export default function LeaguesStep({
  nations,
  choices,
  onChange,
  onCancel,
  onNext,
  error,
}: {
  nations: LeagueNation[]
  choices: LeagueChoice[]
  onChange: (c: LeagueChoice[]) => void
  onCancel: () => void
  onNext: () => void
  error: string | null
}) {
  const mode = (id: number) => choices.find((c) => c.nationId === id)?.mode ?? 'background'
  const set = (id: number, m: LeagueMode) => onChange([...choices.filter((c) => c.nationId !== id), { nationId: id, mode: m }])
  const all = (m: LeagueMode) => onChange(nations.map((n) => ({ nationId: n.nationId, mode: m })))
  return (
    <Overlay>
      <DialogFrame
        title="Escolher Campeonato(s)"
        width={820}
        onCancel={onCancel}
        cancelLabel="Atrás"
        onOk={onNext}
        okLabel="Seguinte"
        header={
          <div className="flex shrink-0 justify-end gap-1 px-1 pt-2">
            <button className="cm-purple h-6 w-40" onClick={() => all('play')}>
              Escolher Todos
            </button>
            <button className="cm-purple h-6 w-40" onClick={() => all('background')}>
              Não Escolher Todos
            </button>
          </div>
        }
      >
        <div className="max-h-[52vh] overflow-auto">
          {nations.map((n) => {
            const m = mode(n.nationId)
            return (
              <div key={n.nationId} className="flex min-h-[30px] items-center gap-3 border-b border-[#1d3a1f]">
                <span className={`w-44 shrink-0 font-title text-[19px] ${m === 'play' ? 'text-cm-yellow' : 'text-white'}`}>{n.name}</span>
                {MODES.map(([k, label]) => (
                  <button
                    key={k}
                    aria-pressed={m === k}
                    onClick={() => set(n.nationId, k)}
                    className={`cm-option w-28 py-0.5 text-[13px] ${m === k && k === 'play' ? '!text-cm-orange' : ''}`}
                  >
                    {label}
                  </button>
                ))}
                <span className="truncate text-[13px] text-[#bfe6c4]">
                  {n.divisions.join(', ')}
                  {n.calendar ? ' · época de ano civil' : ' · época de agosto a maio'}
                </span>
              </div>
            )
          })}
        </div>
        <div className="pt-2 text-[13px] text-[#bfe6c4]">
          Escolhido: joga-se com tudo (mercado, plantéis, estatísticas) e pode treinar lá. Fundo: só os resultados. Fora: não entra no
          jogo. Menos ligas escolhidas tornam o Continuar Jogo mais rápido.
        </div>
        {error && <div className="mt-2 bg-red-900 px-2 py-1 text-white">{error}</div>}
      </DialogFrame>
    </Overlay>
  )
}
