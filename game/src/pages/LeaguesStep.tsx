import type { ReactNode } from 'react'
import type { LeagueChoice } from '../bindings/LeagueChoice'
import type { LeagueMode } from '../bindings/LeagueMode'
import type { LeagueNation } from '../bindings/LeagueNation'
import type { StartSeason } from '../bindings/StartSeason'

// Passo 1 do Novo Jogo: "Escolher Campeonato(s)" do CM, com as opções do jogo ao lado (como o
// "New Game" do FM08): o nome do jogo, a época em que começa, Jogadores Reais e Definição Atributos.

const MODES: [LeagueMode, string][] = [
  ['play', 'ESCOLHIDO'],
  ['background', 'FUNDO'],
  ['off', 'FORA'],
]

const SEASONS: [StartSeason, string, string][] = [
  ['europe', 'Europa 26/27', 'Começa a 1.7.2026, com o Brasileirão de 2026 a meio.'],
  [
    'brazil',
    'Brasil 2026',
    'Começa a 1.1.2026, com a Europa a meio de 2025/26. O 2º semestre de 2025 joga-se ao criar o jogo (demora um pouco).',
  ],
]

export type GameOptions = { realPlayers: boolean; masking: boolean }

function Box({ title, children, className = '' }: { title: string; children: ReactNode; className?: string }) {
  return (
    <div className={`flex min-h-0 flex-col rounded bg-black/40 ${className}`}>
      <div className="shrink-0 px-3 pb-1 pt-2 font-mono text-[12px] font-bold uppercase tracking-[0.12em] text-cm-orange">{title}</div>
      {children}
    </div>
  )
}

export function SimNao({ value, onChange }: { value: boolean; onChange: (v: boolean) => void }) {
  return (
    <span className="flex gap-1">
      {[true, false].map((v) => (
        <button
          key={String(v)}
          aria-pressed={value === v}
          onClick={() => onChange(v)}
          className={`cm-option h-6 w-14 ${value === v ? '!text-cm-orange' : ''}`}
        >
          {v ? 'Sim' : 'Não'}
        </button>
      ))}
    </span>
  )
}

export default function LeaguesStep({
  nations,
  choices,
  onChange,
  options,
  onOptions,
  season,
  onSeason,
  saveName,
  onSaveName,
}: {
  nations: LeagueNation[]
  choices: LeagueChoice[]
  onChange: (c: LeagueChoice[]) => void
  /** "Jogadores Reais" e "Definição Atributos" (tapar o que o clube não conhece), como no CM. */
  options: GameOptions
  onOptions: (o: GameOptions) => void
  season: StartSeason
  onSeason: (s: StartSeason) => void
  saveName: string
  onSaveName: (s: string) => void
}) {
  const mode = (id: number) => choices.find((c) => c.nationId === id)?.mode ?? 'background'
  const set = (id: number, m: LeagueMode) => onChange([...choices.filter((c) => c.nationId !== id), { nationId: id, mode: m }])
  const all = (m: LeagueMode) => onChange(nations.map((n) => ({ nationId: n.nationId, mode: m })))
  const count = (m: LeagueMode) => nations.filter((n) => mode(n.nationId) === m).length
  // A época do Brasil só faz sentido com uma liga de ano civil escolhida
  const calendar = nations.some((n) => n.calendar && mode(n.nationId) === 'play')
  const seasons = SEASONS.filter(([k]) => k !== 'brazil' || calendar)

  return (
    <div className="flex h-full min-h-0 gap-3">
      <Box title="Escolher Campeonato(s)" className="min-w-0 flex-1">
        <div className="min-h-0 flex-1 overflow-auto px-2">
          <div className="cm-zebra">
            {nations.map((n) => {
              const m = mode(n.nationId)
              return (
                <div key={n.nationId} className="flex min-h-[32px] items-center gap-3 px-2">
                  <span className={`w-44 shrink-0 font-title text-[19px] ${m === 'play' ? 'text-cm-yellow' : 'text-white'}`}>{n.name}</span>
                  <div className="flex shrink-0 gap-1.5">
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
                  </div>
                  <span className="min-w-0 flex-1 truncate text-[13px] text-cm-dlgtext">{n.divisions.join(', ')}</span>
                  <span className="shrink-0 text-[12px] text-cm-dlgtext">{n.calendar ? 'Ano civil' : 'Agosto a Maio'}</span>
                </div>
              )
            })}
          </div>
        </div>
        <div className="flex shrink-0 gap-1 p-2">
          <button className="cm-purple h-7 w-44" onClick={() => all('play')}>
            Escolher Todos
          </button>
          <button className="cm-purple h-7 w-44" onClick={() => all('background')}>
            Não Escolher Todos
          </button>
          <span className="ml-auto self-center text-[13px] text-cm-dlgtext">
            Escolhido: joga-se com tudo e pode treinar lá. Fundo: só os resultados. Fora: não entra no jogo.
          </span>
        </div>
      </Box>

      <div className="flex w-[360px] shrink-0 flex-col gap-3">
        <Box title="Jogo">
          <div className="space-y-2 px-3 pb-3">
            <label className="flex items-center gap-3">
              <span className="w-32 shrink-0 text-[14px] text-cm-dlgtext">Nome do Jogo</span>
              <input className="cm-field h-7 min-w-0 flex-1 text-[15px]" value={saveName} onChange={(e) => onSaveName(e.target.value)} />
            </label>
            <div className="flex items-center gap-3">
              <span className="w-32 shrink-0 text-[14px] text-cm-dlgtext">Jogadores Reais</span>
              <SimNao value={options.realPlayers} onChange={(v) => onOptions({ ...options, realPlayers: v })} />
            </div>
            <div className="flex items-center gap-3">
              <span className="w-32 shrink-0 text-[14px] text-cm-dlgtext">Definição Atributos</span>
              <SimNao value={options.masking} onChange={(v) => onOptions({ ...options, masking: v })} />
            </div>
            <div className="text-[12px] leading-snug text-cm-dlgtext">
              Com a Definição Atributos, só se vê o que o clube conhece de cada jogador (os olheiros descobrem o resto).
            </div>
          </div>
        </Box>
        <Box title="Iniciar Época">
          <div className="space-y-1.5 px-3 pb-3">
            {seasons.map(([k, label, text]) => (
              <button
                key={k}
                onClick={() => onSeason(k)}
                className={`block w-full rounded border-2 bg-black/50 px-3 py-1.5 text-left ${
                  season === k ? 'border-cm-yellow' : 'border-transparent hover:border-cm-yellow/40'
                }`}
              >
                <div className={`font-title text-[19px] ${season === k ? 'text-cm-yellow' : 'text-white'}`}>{label}</div>
                <div className="text-[12px] leading-snug text-cm-dlgtext">{text}</div>
              </button>
            ))}
          </div>
        </Box>
        <Box title="Resumo" className="flex-1">
          <div className="space-y-1 px-3 pb-3 text-[14px]">
            {(
              [
                ['Escolhidos', count('play')],
                ['De fundo', count('background')],
                ['Fora', count('off')],
              ] as const
            ).map(([k, v]) => (
              <div key={k} className="flex justify-between">
                <span className="text-cm-dlgtext">{k}</span>
                <span className="font-bold text-cm-yellow">{v}</span>
              </div>
            ))}
            <div className="pt-1 text-[12px] leading-snug text-cm-dlgtext">
              Menos campeonatos escolhidos tornam o Continuar Jogo mais rápido.
            </div>
          </div>
        </Box>
      </div>
    </div>
  )
}
