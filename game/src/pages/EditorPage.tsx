import { useEffect, useMemo, useState } from "react"
import { api } from "../api"
import type { EditorAttribute } from "../bindings/EditorAttribute"
import type { EditorClub } from "../bindings/EditorClub"
import type { EditorCommand } from "../bindings/EditorCommand"
import type { EditorHit } from "../bindings/EditorHit"
import type { EditorPlayer } from "../bindings/EditorPlayer"
import type { EditorProfile } from "../bindings/EditorProfile"
import type { EditorProfileView } from "../bindings/EditorProfileView"
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

const POSITION_LABELS: Record<string, string> = {
  goalkeeper: "Guarda-Redes",
  sweeper: "Líbero",
  defender: "Defesa",
  defensive_midfielder: "Méd. Defensivo",
  midfielder: "Médio",
  attacking_midfielder: "Méd. Ofensivo",
  attacker: "Avançado",
  wing_back: "Ala",
  right_side: "Direita",
  left_side: "Esquerda",
  central: "Centro",
  free_role: "Livre",
}
/** Colunas das posições (chaves) como no editor do CM. */
const POS_LEFT = ["goalkeeper", "sweeper", "defender", "defensive_midfielder", "midfielder", "attacking_midfielder", "attacker", "wing_back", "free_role"]
const POS_RIGHT = ["right_side", "left_side", "central"]

const STATUS: [number, string][] = [
  [1, "Profissional"],
  [2, "Semiprofissional"],
  [3, "Amador"],
]
const ROW = "flex h-7 items-center gap-2 px-2 text-[14px] text-white even:bg-black/20"
const TAB = (on: boolean) => `h-7 px-4 text-[13px] ${on ? "cm-purple" : "bg-black/40 text-[#d8c8f4] hover:bg-black/60"}`
const BOX = "rounded border border-white/15 bg-black/20 p-2"

function NumField({ label, value, onChange, min = 0, max = 999999999, width = "w-28" }: { label: string; value: number; onChange: (v: number) => void; min?: number; max?: number; width?: string }) {
  return (
    <label className={ROW}>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      <input type="number" min={min} max={max} value={value} onChange={(e) => onChange(Number(e.target.value) || 0)} className={`cm-field h-6 ${width} text-right text-[14px]`} />
    </label>
  )
}

function ColorField({ label, value, onChange }: { label: string; value: string | null; onChange: (v: string) => void }) {
  return (
    <label className={ROW}>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      <input value={value ?? ""} placeholder="#rrggbb" onChange={(e) => onChange(e.target.value)} className="cm-field h-6 w-24 text-center font-mono text-[13px]" />
      <span className="h-5 w-12 rounded border border-black/60" style={{ background: value || "transparent" }} />
    </label>
  )
}

function Header({ children }: { children: React.ReactNode }) {
  return <div className="mb-0.5 bg-[#3d1a73] px-2 font-mono text-[11px] font-bold uppercase leading-6 tracking-[0.1em] text-cm-yellow">{children}</div>
}

/** Data em três caixas (dia, mês, ano) como no editor do CM; devolve "AAAA-MM-DD" (o core valida). */
function DateField({ label, value, onChange }: { label: string; value: string | null; onChange: (v: string | null) => void }) {
  const [y = "", m = "", d = ""] = (value ?? "").split("-")
  const set = (yy: string, mm: string, dd: string) => onChange(yy || mm || dd ? `${yy}-${mm.padStart(2, "0")}-${dd.padStart(2, "0")}` : null)
  const box = "cm-field h-6 text-center text-[14px]"
  return (
    <div className={ROW}>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      <input className={`${box} w-12`} inputMode="numeric" maxLength={2} value={d === "" ? "" : String(Number(d))} placeholder="dd" onChange={(e) => set(y, m, e.target.value.replace(/\D/g, ""))} />
      <input className={`${box} w-12`} inputMode="numeric" maxLength={2} value={m === "" ? "" : String(Number(m))} placeholder="mm" onChange={(e) => set(y, e.target.value.replace(/\D/g, ""), d)} />
      <input className={`${box} w-20`} inputMode="numeric" maxLength={4} value={y} placeholder="aaaa" onChange={(e) => set(e.target.value.replace(/\D/g, ""), m, d)} />
    </div>
  )
}

function NationSelect({ label, value, nations, onChange }: { label: string; value: number | null; nations: { id: number; name: string }[]; onChange: (v: number | null) => void }) {
  return (
    <label className={ROW}>
      <span className="w-24">{label}</span>
      <select className="cm-field h-6 min-w-0 flex-1 text-[14px]" value={value ?? ""} onChange={(e) => onChange(e.target.value ? Number(e.target.value) : null)}>
        <option value="">—</option>
        {nations.map((n) => (
          <option key={n.id} value={n.id}>
            {n.name}
          </option>
        ))}
      </select>
    </label>
  )
}

type Kind = "players" | "clubs"
type Tab = "data" | "attr1" | "attr2" | "contract"
type Sub = "details" | "unhappy" | "club"

/** Diálogo "Editar Clube" (como o Edit Club do editor do CM) com |< << >> >| entre os clubes da lista. */
function ClubDialog({
  file,
  clubId,
  ids,
  onClose,
  onSquad,
  onSwitch,
}: {
  file: string
  clubId: number
  ids: number[]
  onClose: () => void
  onSquad: (id: number) => void
  onSwitch: (id: number) => void
}) {
  const [saved, setSaved] = useState<EditorClub | null>(null)
  const [draft, setDraft] = useState<EditorClub | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    let live = true
    api.editorClub(file, clubId).then(
      (c) => {
        if (!live) return
        setSaved(c)
        setDraft(c)
        setError(null)
        setNotice(null)
      },
      (e) => live && setError(String(e)),
    )
    return () => {
      live = false
    }
  }, [file, clubId])

  const dirty = !!saved && !!draft && JSON.stringify(saved) !== JSON.stringify(draft)
  const set = (patch: Partial<EditorClub>) => setDraft((d) => (d ? { ...d, ...patch } : d))

  const update = async (): Promise<boolean> => {
    if (!draft) return false
    setBusy(true)
    setError(null)
    try {
      const c = await api.editorSetClub(file, draft)
      setSaved(c)
      setDraft(c)
      setNotice("Gravado.")
      return true
    } catch (e) {
      setError(String(e))
      return false
    } finally {
      setBusy(false)
    }
  }
  const go = async (id: number | undefined) => {
    if (id === undefined || id === clubId) return
    if (dirty && !(await update())) return
    onSwitch(id)
  }
  const at = ids.indexOf(clubId)
  const nav = at >= 0 && ids.length > 1

  return (
    <Overlay>
      <DialogFrame title="Editar Clube" full={false} width={760} onCancel={undefined}>
        <ErrorBox className="mb-2">{error}</ErrorBox>
        {!draft ? (
          <Loading title="Clube" detail="A abrir…" />
        ) : (
          <div className="flex flex-col gap-2 text-white">
            <div className="flex items-baseline gap-3">
              <span className="font-title text-[22px]">{draft.name}</span>
              {notice && <span className="text-[13px] text-cm-yellow">{notice}</span>}
              <button className="cm-purple ml-auto h-7 px-3 text-[13px]" onClick={() => onSquad(draft.id)}>
                Listar jogadores na janela principal
              </button>
            </div>
            <div className="flex gap-2">
              <div className={`${BOX} min-w-0 flex-1`}>
                <Header>Dados gerais</Header>
                <label className={ROW}>
                  <span className="flex-1">Estatuto profissional</span>
                  <select className="cm-field h-6 w-40 text-[14px]" value={draft.professionalStatus} onChange={(e) => set({ professionalStatus: Number(e.target.value) })}>
                    {STATUS.map(([v, n]) => (
                      <option key={v} value={v}>
                        {n}
                      </option>
                    ))}
                  </select>
                </label>
                <NumField label="Treino (1–20)" value={draft.training} min={1} max={20} onChange={(v) => set({ training: v })} />
                <NumField label="Reputação" value={draft.reputation} max={10000} onChange={(v) => set({ reputation: v })} />
                <NumField label="Saldo bancário" value={draft.cash} onChange={(v) => set({ cash: v })} width="w-36" />
              </div>
              <div className={`${BOX} min-w-0 flex-1`}>
                <Header>Estádio · {draft.stadiumName ?? "sem estádio"}</Header>
                <NumField label="Capacidade" value={draft.capacity} max={200000} onChange={(v) => set({ capacity: v })} />
                <NumField label="Lugares sentados" value={draft.seating} max={200000} onChange={(v) => set({ seating: v })} />
                <NumField label="Expansão" value={draft.expansion} max={200000} onChange={(v) => set({ expansion: v })} />
                <label className={ROW}>
                  <span className="flex-1">Aquecimento subterrâneo</span>
                  <input type="checkbox" checked={draft.undersoilHeating} onChange={(e) => set({ undersoilHeating: e.target.checked })} />
                </label>
              </div>
            </div>
            <div className={BOX}>
              <Header>Equipamentos</Header>
              <div className="flex gap-3">
                <div className="flex-1">
                  <ColorField label="Principal · texto" value={draft.homeFg} onChange={(v) => set({ homeFg: v })} />
                  <ColorField label="Principal · fundo" value={draft.homeBg} onChange={(v) => set({ homeBg: v })} />
                </div>
                <div className="flex-1">
                  <ColorField label="Alternativo · texto" value={draft.awayFg} onChange={(v) => set({ awayFg: v })} />
                  <ColorField label="Alternativo · fundo" value={draft.awayBg} onChange={(v) => set({ awayBg: v })} />
                </div>
              </div>
            </div>
            <div className="flex gap-1">
              <button className="cm-purple h-8 w-16 disabled:opacity-40" disabled={!nav || at === 0} onClick={() => void go(ids[0])}>
                |&lt;
              </button>
              <button className="cm-purple h-8 w-16 disabled:opacity-40" disabled={!nav || at <= 0} onClick={() => void go(ids[at - 1])}>
                &lt;&lt;
              </button>
              <button className="cm-purple h-8 w-16 disabled:opacity-40" disabled={!nav || at >= ids.length - 1} onClick={() => void go(ids[at + 1])}>
                &gt;&gt;
              </button>
              <button className="cm-purple h-8 w-16 disabled:opacity-40" disabled={!nav || at >= ids.length - 1} onClick={() => void go(ids[ids.length - 1])}>
                &gt;|
              </button>
              <span className="flex-1" />
              <button className="cm-purple h-8 w-28 disabled:opacity-40 enabled:text-cm-yellow" disabled={!dirty || busy} onClick={() => void update()}>
                Actualizar
              </button>
              <button
                className="cm-purple h-8 w-28"
                onClick={() => {
                  void (async () => {
                    if (!dirty || (await update())) onClose()
                  })()
                }}
              >
                OK
              </button>
            </div>
          </div>
        )}
      </DialogFrame>
    </Overlay>
  )
}

/** "Editor de Jogos Salvos": no formato do editor do CM (lista à esquerda, abas à direita). */
/** O "Custom Command" do editor do CM: os comandos marcados aplicam-se à lista da esquerda (ou só
 * ao jogador aberto), de uma vez. Quais existem e o que fazem decide o core. */
function CommandDialog({
  count,
  person,
  onRun,
  onClose,
}: {
  count: number
  person: string | null
  onRun: (commands: string[], firstTeam: boolean, onlyPerson: boolean) => void
  onClose: () => void
}) {
  const [list, setList] = useState<EditorCommand[]>([])
  const [failed, setFailed] = useState<string | null>(null)
  const [picked, setPicked] = useState<string[]>([])
  const [firstTeam, setFirstTeam] = useState(false)
  const [onlyPerson, setOnlyPerson] = useState(false)
  useEffect(() => {
    api.editorCommands().then(setList, (e) => setFailed(String(e)))
  }, [])
  const toggle = (key: string, on: boolean) => setPicked((p) => (on ? [...p.filter((k) => k !== key), key] : p.filter((k) => k !== key)))
  return (
    <Overlay>
      <DialogFrame
        title="Comando Personalizado"
        width={460}
        onCancel={onClose}
        onOk={picked.length ? () => onRun(picked, firstTeam, onlyPerson) : undefined}
        okLabel="OK"
      >
        {failed && (
          <div className="p-2 text-[13px] text-[#ff8080]">
            Não foi possível ler os comandos ({failed}). Se o jogo corre no navegador, reinicie a ponte (web.ps1).
          </div>
        )}
        <div className="max-h-[420px] overflow-auto">
          {list.map((c) => (
            <label key={c.key} className="flex h-6 items-center gap-2 px-1 text-[14px] text-white hover:bg-black/30">
              <input type="checkbox" checked={picked.includes(c.key)} onChange={(e) => toggle(c.key, e.target.checked)} />
              {c.label}
            </label>
          ))}
        </div>
        <div className="mt-2 border-t border-white/15 pt-2">
          <label className="flex h-6 items-center gap-2 px-1 text-[14px] text-cm-yellow">
            <input type="checkbox" checked={firstTeam} onChange={(e) => setFirstTeam(e.target.checked)} />
            Só o plantel principal?
          </label>
          <label className={`flex h-6 items-center gap-2 px-1 text-[14px] ${person ? "text-cm-yellow" : "text-[#777]"}`}>
            <input type="checkbox" disabled={!person} checked={onlyPerson} onChange={(e) => setOnlyPerson(e.target.checked)} />
            Só a pessoa actual?{person ? ` (${person})` : ""}
          </label>
          <div className="px-1 pt-1 text-[12px] text-[#9a9aa6]">
            {onlyPerson && person ? `Aplica a ${person}.` : `Aplica aos ${count} jogadores da lista.`}
          </div>
        </div>
      </DialogFrame>
    </Overlay>
  )
}

export default function EditorPage() {
  const game = useGame()
  const [saves, setSaves] = useState<SaveSummary[]>([])
  const [file, setFile] = useState("")
  const [kind, setKind] = useState<Kind>("players")
  const [tab, setTab] = useState<Tab>("data")
  const [sub, setSub] = useState<Sub>("details")
  const [query, setQuery] = useState("")
  const [hits, setHits] = useState<EditorHit[]>([])
  const [player, setPlayer] = useState<EditorPlayer | null>(null)
  const [draft, setDraft] = useState<Record<string, number>>({})
  const [profile, setProfile] = useState<EditorProfileView | null>(null)
  const [profileDraft, setProfileDraft] = useState<EditorProfile | null>(null)
  const [clubOpen, setClubOpen] = useState<number | null>(null)
  const [commandOpen, setCommandOpen] = useState(false)
  const [selected, setSelected] = useState<number | null>(null)
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

  const reset = () => {
    setHits([])
    setPlayer(null)
    setProfile(null)
    setProfileDraft(null)
    setClubOpen(null)
    setSelected(null)
    setDraft({})
    setNotice(null)
  }

  const search = async () => {
    if (!file) return
    const found = await run(() => (kind === "players" ? api.editorSearch(file, query) : api.editorSearchClubs(file, query)))
    if (found) {
      setHits(found)
      setNotice(found.length ? null : kind === "players" ? "Nenhum jogador encontrado." : "Nenhum clube encontrado.")
    }
  }

  /** Lista todos os jogadores do clube e passa para o modo Jogadores. */
  const showSquad = async (clubId: number) => {
    const found = await run(() => api.editorSquad(file, clubId))
    if (found) {
      reset()
      setKind("players")
      setHits(found)
      setNotice(found.length ? null : "O clube não tem jogadores.")
    }
  }

  const open = async (id: number) => {
    if (kind === "clubs") {
      setSelected(id)
      setClubOpen(id)
      return
    }
    const both = await run(() => Promise.all([api.editorPlayer(file, id), api.editorProfile(file, id)]))
    if (both) {
      setSelected(id)
      setNotice(null)
      setPlayer(both[0])
      setDraft({})
      setProfile(both[1])
      setProfileDraft(both[1].profile)
    }
  }

  const changes = useMemo(() => (player ? player.attributes.filter((a) => draft[a.key] !== undefined && draft[a.key] !== a.shown) : []), [player, draft])
  const profileChanged = !!profile && !!profileDraft && JSON.stringify(profile.profile) !== JSON.stringify(profileDraft)
  const dirty = changes.length > 0 || profileChanged

  const update = async () => {
    if (!player) return
    // A ficha primeiro: se o core a recusar, os atributos ficam por gravar e o erro aparece
    if (profileChanged && profileDraft) {
      const v = await run(() => api.editorSetProfile(file, player.id, profileDraft))
      if (!v) return
      setProfile(v)
      setProfileDraft(v.profile)
    }
    if (changes.length) {
      const p = await run(() => api.editorSetAttributes(file, player.id, changes.map((a) => ({ key: a.key, value: draft[a.key] }))))
      if (!p) return
      setPlayer(p)
      setDraft({})
    }
    setNotice(`${player.name}: gravado.`)
  }

  /** Max/Min/Igualar: aplica a todos os atributos do conjunto (Igualar = a média arredondada). */
  const setAll = (group: (a: EditorAttribute) => boolean, how: "max" | "min" | "equal") => {
    if (!player) return
    const list = player.attributes.filter(group)
    const mean = Math.round(list.reduce((s, a) => s + (draft[a.key] ?? a.shown), 0) / Math.max(1, list.length))
    const value = how === "max" ? 20 : how === "min" ? 1 : mean
    setDraft((d) => {
      const next = { ...d }
      for (const a of list) next[a.key] = value
      return next
    })
  }

  /** Corre o Comando Personalizado e reabre o jogador aberto, para se ver o que mudou. */
  const runCommand = async (commands: string[], firstTeam: boolean, onlyPerson: boolean) => {
    setCommandOpen(false)
    const ids = onlyPerson && player ? [player.id] : hits.map((h) => h.id)
    const n = await run(() => api.editorCommand(file, ids, commands, firstTeam))
    if (n === undefined) return
    if (player) await open(player.id)
    setNotice(`Comando aplicado a ${n} jogador${n === 1 ? "" : "es"}.`)
  }

  const pd = profileDraft
  const setP = (patch: Partial<EditorProfile>) => setProfileDraft((d) => (d ? { ...d, ...patch } : d))
  const setUnhappy = (club: boolean, mode: "all" | "none") => {
    if (!profile || !pd) return
    const keys = profile.reasons.filter((r) => r.club === club).map((r) => r.key)
    const rest = pd.unhappy.filter((k) => !keys.includes(k))
    setP({ unhappy: mode === "all" ? [...rest, ...keys] : rest })
  }
  const toggleReason = (key: string, on: boolean) => {
    if (!pd) return
    setP({ unhappy: on ? [...pd.unhappy.filter((k) => k !== key), key] : pd.unhappy.filter((k) => k !== key) })
  }

  const mentalOf = (a: EditorAttribute) => a.group === "mental"
  const playingOf = (a: EditorAttribute) => a.group !== "mental"
  // Os atributos de jogo, por ordem alfabética do rótulo, repartidos em três colunas como no print
  const playing = useMemo(() => (player ? player.attributes.filter(playingOf).sort((a, b) => (LABELS[a.key] ?? a.key).localeCompare(LABELS[b.key] ?? b.key, "pt")) : []), [player])
  const per = Math.ceil(playing.length / 3)
  const columns = [playing.slice(0, per), playing.slice(per, per * 2), playing.slice(per * 2)]
  const attrRow = (a: EditorAttribute) => <AttributeRow key={a.key} a={a} value={draft[a.key] ?? a.shown} onChange={(v) => setDraft((d) => ({ ...d, [a.key]: v }))} />

  const reasonsBox = (club: boolean) =>
    profile && pd ? (
      <div className={BOX}>
        <Header>{club ? "Estado de contentamento com o clube" : "Estado de insatisfação"}</Header>
        <div className="grid grid-cols-2 gap-x-6">
          {profile.reasons
            .filter((r) => r.club === club)
            .map((r) => (
              <label key={r.key} className="flex h-7 items-center gap-2 text-[14px] text-cm-yellow">
                <input type="checkbox" checked={pd.unhappy.includes(r.key)} onChange={(e) => toggleReason(r.key, e.target.checked)} />
                {r.label}
              </label>
            ))}
        </div>
        <div className="mt-2 flex justify-end gap-1">
          <button className="cm-purple h-7 w-28 text-[13px]" onClick={() => setUnhappy(club, "all")}>
            Marcar todos
          </button>
          <button className="cm-purple h-7 w-28 text-[13px]" onClick={() => setUnhappy(club, "none")}>
            Limpar todos
          </button>
        </div>
      </div>
    ) : null

  return (
    <Overlay>
      <DialogFrame title="Editor de Jogos Salvos" full>
        <ErrorBox className="mb-2">{error}</ErrorBox>
        {saves.length === 0 ? (
          <div className="p-6 text-center text-cm-yellow">Nenhum jogo gravado para editar.</div>
        ) : (
          <div className="flex h-full gap-3">
            <div className="flex w-80 shrink-0 flex-col gap-1.5">
              <select className="cm-field h-7 text-[14px]" value={file} onChange={(e) => (setFile(e.target.value), reset())}>
                {saves.map((s) => (
                  <option key={s.file} value={s.file}>
                    {s.name} · {s.clubName}
                  </option>
                ))}
              </select>
              <div className="min-h-[200px] flex-1 overflow-auto rounded border border-white/15 bg-black/30">
                {hits.map((h) => (
                  <button
                    key={h.id}
                    onClick={() => void open(h.id)}
                    className={`flex h-6 w-full items-center gap-2 px-2 text-left text-[13.5px] hover:bg-cm-sel ${selected === h.id ? "bg-cm-sel text-white" : "text-cm-yellow"}`}
                  >
                    <span className="min-w-0 flex-1 truncate">{h.name}</span>
                    <span className="w-9 text-right font-mono text-[12px]">{h.ability ?? ""}</span>
                  </button>
                ))}
                {notice && selected === null && <div className="p-3 text-center text-[13px] text-[#d8c8f4]">{notice}</div>}
              </div>
              <div className="text-[13px] text-cm-yellow">{hits.length} itens encontrados.</div>
              <form
                className="flex flex-col gap-1.5"
                onSubmit={(e) => {
                  e.preventDefault()
                  void search()
                }}
              >
                <input className="cm-field h-7 text-[14px]" placeholder={kind === "players" ? "Nome do jogador" : "Nome do clube"} value={query} onChange={(e) => setQuery(e.target.value)} />
                <div className="flex flex-col gap-0.5 text-[14px] text-white">
                  {(["players", "clubs"] as Kind[]).map((k) => (
                    <label key={k} className="flex items-center gap-2">
                      <input type="radio" checked={kind === k} onChange={() => (setKind(k), reset())} />
                      {k === "players" ? "Jogadores" : "Clubes"}
                    </label>
                  ))}
                </div>
                <button className="cm-purple h-8" disabled={busy}>
                  Procurar
                </button>
              </form>
              <button className="cm-purple h-8 enabled:text-cm-yellow" disabled={!dirty || busy} onClick={() => void update()}>
                Actualizar Jogador
              </button>
              <button className="cm-purple h-8" disabled={kind !== "players" || !hits.length || busy} onClick={() => setCommandOpen(true)}>
                Comando Personalizado
              </button>
              <button className="cm-purple h-8" onClick={() => game.setMode("start")}>
                Sair
              </button>
            </div>
            <div className="min-w-0 flex-1">
              {kind === "clubs" ? (
                <div className="p-8 text-center text-[#d8c8f4]">Procure um clube pelo nome e escolha-o na lista: abre o diálogo Editar Clube.</div>
              ) : !player || !pd || !profile ? (
                <div className="p-8 text-center text-[#d8c8f4]">Procure um jogador pelo nome (ou deixe vazio para os melhores) e escolha-o na lista.</div>
              ) : (
                <>
                  <div className="mb-1.5 flex items-baseline gap-3">
                    <span className="font-title text-[22px] text-white">{player.name}</span>
                    <span className="text-cm-magenta">{player.club ?? "Sem clube"}</span>
                    {notice && <span className="text-[13px] text-cm-yellow">{notice}</span>}
                    <span className="ml-auto font-mono text-[13px] text-cm-yellow">
                      CA {pd.ability} · PA {pd.potential}
                    </span>
                  </div>
                  <div className="mb-1.5 flex gap-1">
                    {(
                      [
                        ["data", "Dados"],
                        ["attr1", "Atributos 1"],
                        ["attr2", "Atributos 2"],
                        ["contract", "Contrato"],
                      ] as [Tab, string][]
                    ).map(([t, label]) => (
                      <button key={t} className={TAB(tab === t)} onClick={() => setTab(t)}>
                        {label}
                      </button>
                    ))}
                  </div>

                  {tab === "data" && (
                    <div className="flex flex-col gap-2">
                      <div className={`${BOX} max-w-xl`}>
                        <Header>Dados da pessoa</Header>
                        <DateField label="Data de nascimento" value={pd.birthDate} onChange={(v) => setP({ birthDate: v })} />
                        <NationSelect label="Nação" value={pd.nationId} nations={profile.nations} onChange={(v) => setP({ nationId: v })} />
                        <NationSelect label="2ª nação" value={pd.secondNationId} nations={profile.nations} onChange={(v) => setP({ secondNationId: v })} />
                        <div className="flex">
                          <div className="flex-1">
                            <NumField label="Internacionalizações" value={pd.caps} max={999} width="w-16" onChange={(v) => setP({ caps: v })} />
                          </div>
                          <div className="flex-1">
                            <NumField label="Golos" value={pd.goals} max={999} width="w-16" onChange={(v) => setP({ goals: v })} />
                          </div>
                        </div>
                      </div>
                      <div className={`${BOX} max-w-xl`}>
                        <Header>Posições (1–20)</Header>
                        <div className="flex gap-6">
                          <div className="flex-1">
                            {POS_LEFT.map((k) => (
                              <NumField key={k} label={POSITION_LABELS[k]} value={pd.positions[profile.positionKeys.indexOf(k)]} min={1} max={20} width="w-16" onChange={(v) => setP({ positions: pd.positions.map((x, j) => (j === profile.positionKeys.indexOf(k) ? Math.min(20, Math.max(1, v)) : x)) })} />
                            ))}
                          </div>
                          <div className="flex-1">
                            {POS_RIGHT.map((k) => (
                              <NumField key={k} label={POSITION_LABELS[k]} value={pd.positions[profile.positionKeys.indexOf(k)]} min={1} max={20} width="w-16" onChange={(v) => setP({ positions: pd.positions.map((x, j) => (j === profile.positionKeys.indexOf(k) ? Math.min(20, Math.max(1, v)) : x)) })} />
                            ))}
                          </div>
                        </div>
                      </div>
                      {player.clubId != null && (
                        <div className="flex max-w-xl gap-1">
                          <button className="cm-purple h-8 flex-1" onClick={() => void showSquad(player.clubId!)}>
                            Plantel e Staff: {player.club}
                          </button>
                          <button className="cm-purple h-8 flex-1" onClick={() => setClubOpen(player.clubId)}>
                            Editar Clube: {player.club}
                          </button>
                        </div>
                      )}
                    </div>
                  )}

                  {tab === "attr1" && (
                    <>
                      <div className={BOX}>
                        <Header>Atributos de jogo</Header>
                        <div className="flex gap-4">
                          {columns.map((col, i) => (
                            <div key={i} className="min-w-0 flex-1">
                              {col.map(attrRow)}
                            </div>
                          ))}
                          <div className="flex w-36 shrink-0 flex-col gap-1">
                            <div className="flex items-center gap-2 text-[13px] text-white">
                              <span className="h-5 w-8 rounded bg-white/90" /> = Oculto
                            </div>
                            <button className="cm-purple h-7 text-[13px]" onClick={() => setAll(playingOf, "equal")}>
                              Igualar Todos
                            </button>
                            <button className="cm-purple h-7 text-[13px]" onClick={() => setAll(playingOf, "max")}>
                              Máx. Todos
                            </button>
                            <button className="cm-purple h-7 text-[13px]" onClick={() => setAll(playingOf, "min")}>
                              Mín. Todos
                            </button>
                            {pd.morale != null && <NumField label="Moral" value={pd.morale} min={1} max={20} width="w-14" onChange={(v) => setP({ morale: Math.min(20, Math.max(1, v)) })} />}
                          </div>
                        </div>
                      </div>
                      <div className={`${BOX} mt-2`}>
                        <Header>Reputações e habilidades</Header>
                        <div className="grid grid-cols-2 gap-x-6">
                          <NumField label="Reputação local" value={pd.homeReputation} max={10000} onChange={(v) => setP({ homeReputation: v })} />
                          <NumField label="Capacidade actual (CA)" value={pd.ability} max={200} onChange={(v) => setP({ ability: v })} />
                          <NumField label="Reputação actual" value={pd.currentReputation} max={10000} onChange={(v) => setP({ currentReputation: v })} />
                          <NumField label="Potencial (PA)" value={pd.potential} max={200} onChange={(v) => setP({ potential: v })} />
                          <NumField label="Reputação mundial" value={pd.worldReputation} max={10000} onChange={(v) => setP({ worldReputation: v })} />
                        </div>
                      </div>
                      <div className="pt-1.5 text-[12px] text-[#8a8a8a]">Os valores são os que o jogo mostra (1 a 20); à direita, o valor guardado no save. Os destacados a branco o jogo esconde.</div>
                    </>
                  )}

                  {tab === "attr2" && (
                    <div className={`${BOX} max-w-xl`}>
                      <Header>Atributos mentais</Header>
                      <div className="grid grid-cols-2 gap-x-6">{player.attributes.filter(mentalOf).map(attrRow)}</div>
                      <div className="mt-2 flex justify-end gap-1">
                        <button className="cm-purple h-7 w-28 text-[13px]" onClick={() => setAll(mentalOf, "max")}>
                          Máx. Todos
                        </button>
                        <button className="cm-purple h-7 w-28 text-[13px]" onClick={() => setAll(mentalOf, "min")}>
                          Mín. Todos
                        </button>
                      </div>
                    </div>
                  )}

                  {tab === "contract" && (
                    <>
                      <div className="mb-1.5 flex gap-1">
                        {(
                          [
                            ["details", "Detalhes do Contrato"],
                            ["unhappy", "Insatisfação"],
                            ["club", "Insatisfação com o Clube"],
                          ] as [Sub, string][]
                        ).map(([s, label]) => (
                          <button key={s} className={TAB(sub === s)} onClick={() => setSub(s)}>
                            {label}
                          </button>
                        ))}
                      </div>
                      {sub === "details" && (
                        <div className={`${BOX} max-w-xl`}>
                          <Header>Detalhes do contrato</Header>
                          <NumField label="Salário semanal" value={pd.wage} max={100000000} width="w-36" onChange={(v) => setP({ wage: v })} />
                          <DateField label="Início" value={pd.contractStart} onChange={(v) => setP({ contractStart: v })} />
                          <DateField label="Termina" value={pd.contractExpires} onChange={(v) => setP({ contractExpires: v })} />
                        </div>
                      )}
                      {sub === "unhappy" && reasonsBox(false)}
                      {sub === "club" && reasonsBox(true)}
                    </>
                  )}
                </>
              )}
            </div>
          </div>
        )}
      </DialogFrame>
      {busy && <Loading title="Editor" detail="A trabalhar no jogo gravado…" />}
      {clubOpen !== null && (
        <ClubDialog
          file={file}
          clubId={clubOpen}
          ids={kind === "clubs" ? hits.map((h) => h.id) : []}
          onClose={() => setClubOpen(null)}
          onSquad={(id) => {
            setClubOpen(null)
            void showSquad(id)
          }}
          onSwitch={(id) => (setClubOpen(id), setSelected(id))}
        />
      )}
      {commandOpen && (
        <CommandDialog
          count={hits.length}
          person={player?.name ?? null}
          onClose={() => setCommandOpen(false)}
          onRun={(commands, firstTeam, onlyPerson) => void runCommand(commands, firstTeam, onlyPerson)}
        />
      )}
    </Overlay>
  )
}
