import { useEffect, useMemo, useState } from "react"
import { api } from "../api"
import type { EditorAttribute } from "../bindings/EditorAttribute"
import type { EditorHit } from "../bindings/EditorHit"
import type { EditorPlayer } from "../bindings/EditorPlayer"
import type { SaveSummary } from "../bindings/SaveSummary"
import { DialogFrame, Overlay } from "../components/Dialog"
import Loading from "../components/Loading"
import { ErrorBox } from "../components/Screen"
import { useGame } from "../game"

// Editor de jogos salvos (marco 3.20, primeira fatia): procura um jogador num save fechado e
// edita os atributos pelo valor de 1 a 20 que o jogo mostra. O core converte para o valor
// guardado (ao lado, só de leitura); os atributos ocultos vão a branco, como no editor do CM.

const LABELS: Record<string, string> = {
  acceleration: "Aceleração",
  adaptability: "Adaptabilidade",
  aggression: "Agressividade",
  agility: "Agilidade",
  ambition: "Ambição",
  anticipation: "Antecipação",
  balance: "Balanço",
  bravery: "Bravura",
  consistency: "Consistência",
  corners: "Cantos",
  crossing: "Cruzamentos",
  decisions: "Decisões",
  determination: "Determinação",
  dirtiness: "Jogo Sujo",
  dribbling: "Finta",
  finishing: "Finalização",
  flair: "Instinto",
  free_kicks: "Bolas Paradas",
  handling: "Jogo Mãos",
  heading: "Cabeceamento",
  important_matches: "Jogos Importantes",
  injury_proneness: "Propensão a Lesões",
  jumping: "Impulsão",
  leadership: "Influência",
  left_foot: "Pé Esquerdo",
  long_shots: "Remates Longe",
  loyalty: "Lealdade",
  marking: "Marcação",
  natural_fitness: "Condição Física Natural",
  off_the_ball: "Sem Bola",
  one_on_ones: "Um-contra-um",
  pace: "Velocidade",
  passing: "Passe",
  penalties: "Penáltis",
  positioning: "Posicionamento",
  pressure: "Pressão",
  professionalism: "Profissionalismo",
  reflexes: "Reflexos",
  right_foot: "Pé Direito",
  sportsmanship: "Desportivismo",
  stamina: "Resistência",
  strength: "Força",
  tackling: "Desarme",
  teamwork: "Trabalho Equipa",
  technique: "Técnica",
  temperament: "Temperamento",
  throw_ins: "Lançamentos",
  versatility: "Versatilidade",
  vision: "Criatividade",
  work_rate: "Índice Trabalho",
}

const GROUPS: [string, string][] = [
  ["technical", "Técnicos"],
  ["other", "Físicos e Outros"],
  ["mental", "Mentais"],
]

function AttributeRow({ a, value, onChange }: { a: EditorAttribute; value: number; onChange: (v: number) => void }) {
  const changed = value !== a.shown
  return (
    <div className={`flex h-7 items-center gap-2 px-2 text-[14px] ${a.hidden ? "rounded bg-white/90 text-black" : "even:bg-black/20 text-white"}`}>
      <span className="min-w-0 flex-1 truncate">{LABELS[a.key] ?? a.key}</span>
      <input
        type="number"
        min={1}
        max={20}
        value={value}
        onChange={(e) => onChange(Math.min(20, Math.max(1, Number(e.target.value) || 1)))}
        className={`cm-field h-6 w-14 text-center text-[14px] ${changed ? "!text-cm-yellow" : ""}`}
      />
      <span className="w-10 text-right font-mono text-[12px] text-[#8a8a8a]" title="Valor guardado no save (só leitura)">
        {a.relative ? a.stored : ""}
      </span>
    </div>
  )
}

/** "Editor de Jogos Salvos": escolhe o save, procura o jogador e edita os atributos. */
export default function EditorPage() {
  const game = useGame()
  const [saves, setSaves] = useState<SaveSummary[]>([])
  const [file, setFile] = useState("")
  const [query, setQuery] = useState("")
  const [hits, setHits] = useState<EditorHit[]>([])
  const [player, setPlayer] = useState<EditorPlayer | null>(null)
  const [draft, setDraft] = useState<Record<string, number>>({})
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    api.listSaves().then(
      (list) => {
        setSaves(list)
        setFile((f) => f || list[0]?.file || "")
      },
      (e) => setError(String(e)),
    )
  }, [])

  const run = async <T,>(job: () => Promise<T>): Promise<T | undefined> => {
    setBusy(true)
    setError(null)
    try {
      return await job()
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(false)
    }
  }

  const search = async () => {
    if (!file || !query.trim()) return
    const found = await run(() => api.editorSearch(file, query))
    if (found) {
      setHits(found)
      setNotice(found.length ? null : "Nenhum jogador encontrado.")
    }
  }

  const open = async (id: number) => {
    const p = await run(() => api.editorPlayer(file, id))
    if (p) {
      setPlayer(p)
      setDraft({})
      setNotice(null)
    }
  }

  const changes = useMemo(
    () => (player ? player.attributes.filter((a) => draft[a.key] !== undefined && draft[a.key] !== a.shown) : []),
    [player, draft],
  )

  const update = async () => {
    if (!player || !changes.length) return
    const p = await run(() => api.editorSetAttributes(file, player.id, changes.map((a) => ({ key: a.key, value: draft[a.key] }))))
    if (p) {
      setPlayer(p)
      setDraft({})
      setNotice(`${p.name}: ${changes.length} atributo(s) gravado(s).`)
    }
  }

  return (
    <Overlay>
      <DialogFrame
        title="Editor de Jogos Salvos"
        width={980}
        onCancel={() => game.setMode("start")}
        cancelLabel="Fechar"
        onOk={changes.length ? () => void update() : undefined}
        okLabel="Actualizar"
      >
        <ErrorBox className="mb-2">{error}</ErrorBox>
        {saves.length === 0 ? (
          <div className="p-6 text-center text-cm-yellow">Nenhum jogo gravado para editar.</div>
        ) : (
          <div className="flex gap-3">
            <div className="flex w-72 shrink-0 flex-col gap-1.5">
              <select className="cm-field h-7 text-[14px]" value={file} onChange={(e) => (setFile(e.target.value), setHits([]), setPlayer(null))}>
                {saves.map((s) => (
                  <option key={s.file} value={s.file}>
                    {s.name} · {s.clubName}
                  </option>
                ))}
              </select>
              <form
                className="flex gap-1"
                onSubmit={(e) => {
                  e.preventDefault()
                  void search()
                }}
              >
                <input className="cm-field h-7 min-w-0 flex-1 text-[14px]" placeholder="Nome do jogador" value={query} onChange={(e) => setQuery(e.target.value)} />
                <button className="cm-purple h-7 w-20" disabled={busy || !query.trim()}>
                  Procurar
                </button>
              </form>
              <div className="max-h-[420px] overflow-auto rounded bg-black/30">
                {hits.map((h) => (
                  <button
                    key={h.id}
                    onClick={() => void open(h.id)}
                    className={`flex h-7 w-full items-center gap-2 px-2 text-left text-[13.5px] hover:bg-cm-sel ${player?.id === h.id ? "bg-cm-sel" : ""}`}
                  >
                    <span className="min-w-0 flex-1 truncate text-white">{h.name}</span>
                    <span className="max-w-[90px] truncate text-[12px] text-cm-magenta">{h.club ?? "-"}</span>
                    <span className="w-7 text-right font-mono text-[12px] text-cm-yellow">{h.ability ?? ""}</span>
                  </button>
                ))}
                {notice && !player && <div className="p-3 text-center text-[13px] text-[#d8c8f4]">{notice}</div>}
              </div>
            </div>
            <div className="min-w-0 flex-1">
              {!player ? (
                <div className="p-8 text-center text-[#d8c8f4]">Procure um jogador pelo nome e escolha-o na lista.</div>
              ) : (
                <>
                  <div className="mb-1.5 flex items-baseline gap-3">
                    <span className="font-title text-[22px] text-white">{player.name}</span>
                    <span className="text-cm-magenta">{player.club ?? "Sem clube"}</span>
                    <span className="ml-auto font-mono text-[13px] text-cm-yellow">
                      CA {player.ability ?? "-"} · PA {player.potential ?? "-"}
                    </span>
                  </div>
                  {notice && <div className="mb-1 text-[13px] text-cm-yellow">{notice}</div>}
                  <div className="flex gap-3">
                    {GROUPS.map(([group, title]) => (
                      <div key={group} className="min-w-0 flex-1">
                        <div className="mb-0.5 flex bg-[#3d1a73] px-2 font-mono text-[11px] font-bold uppercase tracking-[0.1em] leading-6 text-cm-yellow">
                          <span className="flex-1">{title}</span>
                          <span title="Valor guardado no save">Guard.</span>
                        </div>
                        {player.attributes
                          .filter((a) => a.group === group)
                          .map((a) => (
                            <AttributeRow
                              key={a.key}
                              a={a}
                              value={draft[a.key] ?? a.shown}
                              onChange={(v) => setDraft((d) => ({ ...d, [a.key]: v }))}
                            />
                          ))}
                      </div>
                    ))}
                  </div>
                  <div className="pt-1.5 text-[12px] text-[#8a8a8a]">
                    Os valores são os que o jogo mostra (1 a 20). Os destacados a branco o jogo esconde.
                  </div>
                </>
              )}
            </div>
          </div>
        )}
      </DialogFrame>
      {busy && <Loading title="Editor" detail="A trabalhar no jogo gravado…" />}
    </Overlay>
  )
}
