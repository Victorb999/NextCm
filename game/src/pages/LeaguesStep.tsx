import type { LeagueChoice } from "../bindings/LeagueChoice"
import type { LeagueMode } from "../bindings/LeagueMode"
import type { LeagueNation } from "../bindings/LeagueNation"
import { DialogFrame, Overlay } from "../components/Dialog"
import { ErrorBox } from "../components/Screen"

// "Escolher Campeonato(s)", como no CM: cada nação Escolhida (joga-se com tudo e pode-se
// treinar lá), de Fundo (só os resultados) ou fora do jogo.

const MODES: [LeagueMode, string][] = [
  ["play", "ESCOLHIDO"],
  ["background", "FUNDO"],
  ["off", "FORA"],
]

export default function LeaguesStep({
  nations,
  choices,
  onChange,
  onCancel,
  onNext,
  error,
  options,
  onOptions,
}: {
  nations: LeagueNation[]
  choices: LeagueChoice[]
  /** "Jogadores Reais" e "Definição Atributos" (tapar o que o clube não conhece), como no CM. */
  options: { realPlayers: boolean; masking: boolean }
  onOptions: (o: { realPlayers: boolean; masking: boolean }) => void
  onChange: (c: LeagueChoice[]) => void
  onCancel: () => void
  onNext: () => void
  error: string | null
}) {
  const mode = (id: number) =>
    choices.find((c) => c.nationId === id)?.mode ?? "background"
  const set = (id: number, m: LeagueMode) =>
    onChange([
      ...choices.filter((c) => c.nationId !== id),
      { nationId: id, mode: m },
    ])
  const all = (m: LeagueMode) =>
    onChange(nations.map((n) => ({ nationId: n.nationId, mode: m })))
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
          <div className="flex shrink-0 items-center justify-end gap-1 px-1 pt-2">
            {(
              [
                ["realPlayers", "Jogadores Reais:"],
                ["masking", "Definição Atributos:"],
              ] as const
            ).map(([key, label]) => (
              <span
                key={key}
                className="mr-3 flex items-center gap-1.5 whitespace-nowrap text-[14px] text-white"
              >
                {label}
                {[true, false].map((v) => (
                  <button
                    key={String(v)}
                    onClick={() => onOptions({ ...options, [key]: v })}
                    className={`h-6 px-2 font-bold ${options[key] === v ? "border border-cm-yellow text-cm-orange" : "text-[#8a8a8a] hover:text-white"}`}
                  >
                    {v ? "Sim" : "Não"}
                  </button>
                ))}
              </span>
            ))}
            <div className="flex-1" />
            <button className="cm-purple h-6 w-40" onClick={() => all("play")}>
              Escolher Todos
            </button>
            <button
              className="cm-purple h-6 w-40"
              onClick={() => all("background")}
            >
              Não Escolher Todos
            </button>
          </div>
        }
      >
        <div className="max-h-[52vh] overflow-auto cm-zebra bg-[#00000011]">
          {nations.map((n) => {
            const m = mode(n.nationId)
            return (
              <div
                key={n.nationId}
                className="flex min-h-[30px] items-center gap-3 border-b border-[#1d3a1f] px-2 last:border-b-0"
              >
                <span
                  className={`w-44 shrink-0 font-title text-[19px] ${m === "play" ? "text-cm-yellow" : "text-white"}`}
                >
                  {n.name}
                </span>
                <div className="flex gap-1.5">
                  {MODES.map(([k, label]) => (
                    <button
                      key={k}
                      aria-pressed={m === k}
                      onClick={() => set(n.nationId, k)}
                      className={`cm-option w-28 py-0.5 text-[13px] ${m === k && k === "play" ? "!text-cm-orange" : ""}`}
                    >
                      {label}
                    </button>
                  ))}
                </div>
                <span className="truncate text-[13px] text-cm-dlgtext">
                  {n.divisions.join(", ")}
                  {n.calendar
                    ? " · época de ano civil"
                    : " · época de agosto a maio"}
                </span>
              </div>
            )
          })}
        </div>
        <div className="pt-2 text-[13px] text-cm-dlgtext">
          Escolhido: joga-se com tudo (mercado, plantéis, estatísticas) e pode
          treinar lá. Fundo: só os resultados. Fora: não entra no jogo. Menos
          ligas escolhidas tornam o Continuar Jogo mais rápido.
        </div>
        <ErrorBox className="mt-2">{error}</ErrorBox>
      </DialogFrame>
    </Overlay>
  )
}
