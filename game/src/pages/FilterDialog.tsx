import { useEffect, useState } from "react"
import type { ReactNode } from "react"
import type { NationOption } from "../bindings/NationOption"
import {
  AGE_BANDS,
  EMPTY_FILTERS,
  POSITION_GROUPS,
  SIDES,
  filterAttributes,
  type Filters,
} from "./searchFilters"

function Option({
  selected,
  onClick,
  children,
}: {
  selected: boolean
  onClick: () => void
  children: ReactNode
}) {
  return (
    <button
      aria-pressed={selected}
      onClick={onClick}
      className={`h-8 min-w-[64px] rounded-[4px] border px-3 text-[13px] ${
        selected
          ? "border-cm-yellow bg-cm-dlghover font-bold text-cm-yellow"
          : "border-cm-dlgline text-cm-dlgtext hover:bg-cm-dlghover/60"
      }`}
    >
      {children}
    </button>
  )
}

/** Uma linha do Geral, como no CM: o nome à esquerda e as opções à frente (zebrada pela lista). */
function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex min-h-[44px] items-center gap-3 px-3 py-1.5">
      <div className="w-28 shrink-0 text-[11px] font-bold font-mono uppercase tracking-[0.12em] text-cm-mint">
        {label}
      </div>
      <div className="flex flex-wrap items-center gap-1.5">{children}</div>
    </div>
  )
}

/** "- 7 +": um mínimo da aba Atributos (ou o Igualar). */
function Stepper({
  label,
  value,
  text,
  active,
  colour,
  onChange,
  min,
  max,
}: {
  label: string
  value: number
  text?: string
  active: boolean
  colour: string
  onChange: (v: number) => void
  min: number
  max: number
}) {
  const step = "flex h-6 w-7 items-center justify-center rounded-[3px] bg-cm-dlg font-bold text-white ring-1 ring-cm-dlgline hover:bg-cm-dlghover disabled:text-white/25"
  return (
    <div className="flex h-8 items-center gap-1.5 px-3">
      <span className={`min-w-0 flex-1 truncate ${active ? "font-bold text-cm-yellow" : "text-cm-dlgtext"}`}>{label}</span>
      <button className={step} disabled={value <= min} onClick={() => onChange(value - 1)} aria-label={`Baixar ${label}`}>
        -
      </button>
      <span className={`w-9 text-center font-bold ${colour}`}>{text ?? value}</span>
      <button className={step} disabled={value >= max} onClick={() => onChange(value + 1)} aria-label={`Subir ${label}`}>
        +
      </button>
    </div>
  )
}

/** A aba Atributos: o mínimo de cada um (1 = qualquer) e quantos têm de bater (Igualar). */
function AttributesTab({ f, players, set }: { f: Filters; players: boolean; set: (patch: Partial<Filters>) => void }) {
  const list = filterAttributes(players)
  const chosen = list.filter(([k]) => (f.attributes[k] ?? 1) > 1).length
  const needed = f.needed === null ? chosen : Math.min(f.needed, chosen)
  const cells: ReactNode[] = list.map(([key, label]) => {
    const v = f.attributes[key] ?? 1
    return (
      <Stepper
        key={key}
        label={label}
        value={v}
        active={v > 1}
        colour={v > 1 ? "text-cm-yellow" : "text-cm-green"}
        min={1}
        max={20}
        onChange={(n) => set({ attributes: { ...f.attributes, [key]: n }, needed: null })}
      />
    )
  })
  // O Igualar fica na coluna da direita, na última linha
  if (list.length % 2 === 0) cells.push(<div key="blank" />)
  cells.push(
    <Stepper
      key="needed"
      label="Igualar"
      value={needed}
      text={`${needed}/${chosen}`}
      active={false}
      colour="text-cm-pink"
      min={Math.min(1, chosen)}
      max={chosen}
      onChange={(n) => set({ needed: n })}
    />,
  )
  return <div className="cm-zebra-2 grid grid-cols-2 gap-x-4">{cells}</div>
}

/** Diálogo "Filtros para Pesquisar Jogador": o painel verde do CM, aberto ao lado da tabela. */
export default function FilterDialog({
  initial,
  players,
  nations,
  onCancel,
  onOk,
}: {
  initial: Filters
  players: boolean
  nations: NationOption[]
  onCancel: () => void
  onOk: (f: Filters) => void
}) {
  const [f, setF] = useState(initial)
  const [tab, setTab] = useState<"general" | "attributes">("general")
  const set = (patch: Partial<Filters>) => setF((old) => ({ ...old, ...patch }))
  const brazil = nations.find((n) => n.name === "Brazil")
  const otherNation = f.nationId !== null && f.nationId !== brazil?.id

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel()
      if (e.key === "Enter") onOk(f)
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  })

  return (
    <div className="cm-dialog-in absolute bottom-[60px] right-2.5 top-[72px] z-30 flex w-[620px] flex-col overflow-hidden rounded-lg border-2 border-cm-dlgborder bg-cm-dlg shadow-[0_12px_32px_rgba(0,0,0,0.7)]">
      <div className="flex h-12 shrink-0 items-center gap-2 border-b border-cm-dlgline px-4">
        <span className="size-2 rounded-full bg-cm-mint" />
        <span className="font-bold font-mono uppercase tracking-[0.12em] text-white">
          {players
            ? "Filtros para Pesquisar Jogador"
            : "Filtros para Pesquisar Staff"}
        </span>
      </div>
      <div className="flex shrink-0 gap-1.5 px-4 pt-3">
        <button className={`cm-purple h-7 w-28 ${tab === "general" ? "ring-1 ring-cm-yellow" : ""}`} onClick={() => setTab("general")}>
          Geral
        </button>
        <button className={`cm-purple h-7 w-28 ${tab === "attributes" ? "ring-1 ring-cm-yellow" : ""}`} onClick={() => setTab("attributes")}>
          Atributos
        </button>
        <div className="flex-1" />
        <button
          className="cm-purple h-7 w-28"
          onClick={() => setF(EMPTY_FILTERS)}
        >
          Limpar
        </button>
      </div>
      <div className="min-h-0 flex-1 overflow-auto p-4">
        {tab === "attributes" ? (
          <AttributesTab f={f} players={players} set={set} />
        ) : (
          <div className="cm-zebra">
            <Row label="Texto">
              <Option selected={!f.text} onClick={() => set({ text: "" })}>
                Qualquer
              </Option>
              <input
                autoFocus
                value={f.text}
                onChange={(e) => set({ text: e.target.value })}
                placeholder="Específico"
                className="h-8 w-56 rounded-[4px] border border-cm-dlgline bg-cm-dlg px-2.5 text-cm-yellow outline-none placeholder:text-cm-mint/60 focus:border-cm-yellow"
              />
            </Row>
            <Row label="Nacionalidade">
              <Option
                selected={f.nationId === null}
                onClick={() => set({ nationId: null })}
              >
                Qualquer
              </Option>
              {brazil && (
                <Option
                  selected={f.nationId === brazil.id}
                  onClick={() => set({ nationId: brazil.id })}
                >
                  BRA
                </Option>
              )}
              <select
                value={otherNation ? String(f.nationId) : ""}
                onChange={(e) =>
                  set({ nationId: e.target.value ? Number(e.target.value) : null })
                }
                className={`h-8 rounded-[4px] border bg-cm-dlg px-2 outline-none ${otherNation ? "border-cm-yellow text-cm-yellow" : "border-cm-dlgline text-cm-dlgtext"}`}
              >
                <option value="">Outro…</option>
                {nations.map((n) => (
                  <option key={n.id} value={n.id}>
                    {n.name}
                  </option>
                ))}
              </select>
            </Row>
            <Row label="Idade">
              <Option selected={f.age === null} onClick={() => set({ age: null })}>
                Qualquer
              </Option>
              {AGE_BANDS.map((b) => (
                <Option
                  key={b.key}
                  selected={f.age === b.key}
                  onClick={() => set({ age: b.key })}
                >
                  {b.label}
                </Option>
              ))}
            </Row>
            {players && (
              <>
                <Row label="Posição">
                  <Option
                    selected={f.positionGroup === null}
                    onClick={() => set({ positionGroup: null })}
                  >
                    Qualquer
                  </Option>
                  {POSITION_GROUPS.map((g) => (
                    <Option
                      key={g.key}
                      selected={f.positionGroup === g.key}
                      onClick={() => set({ positionGroup: g.key })}
                    >
                      {g.label}
                    </Option>
                  ))}
                </Row>
                <Row label="Lado">
                  <Option
                    selected={f.side === null}
                    onClick={() => set({ side: null })}
                  >
                    Qualquer
                  </Option>
                  {SIDES.map((s) => (
                    <Option
                      key={s.key}
                      selected={f.side === s.key}
                      onClick={() => set({ side: s.key })}
                    >
                      {s.label}
                    </Option>
                  ))}
                </Row>
                <Row label="Contrato">
                  <Option
                    selected={!f.expiring}
                    onClick={() => set({ expiring: false })}
                  >
                    Qualquer
                  </Option>
                  <Option
                    selected={f.expiring}
                    onClick={() => set({ expiring: true })}
                  >
                    A expirar
                  </Option>
                </Row>
                <Row label="Lesões">
                  <Option
                    selected={f.injured === null}
                    onClick={() => set({ injured: null })}
                  >
                    Qualquer
                  </Option>
                  <Option
                    selected={f.injured === "only"}
                    onClick={() => set({ injured: "only" })}
                  >
                    Lesionados
                  </Option>
                  <Option
                    selected={f.injured === "exclude"}
                    onClick={() => set({ injured: "exclude" })}
                  >
                    Sem lesionados
                  </Option>
                </Row>
                <Row label="Interessados">
                  <Option
                    selected={!f.interested}
                    onClick={() => set({ interested: false })}
                  >
                    Qualquer
                  </Option>
                  <Option
                    selected={f.interested}
                    onClick={() => set({ interested: true })}
                  >
                    Apenas
                  </Option>
                </Row>
                <Row label="Incluir Plantel">
                  <Option
                    selected={f.ownSquad}
                    onClick={() => set({ ownSquad: true })}
                  >
                    Sim
                  </Option>
                  <Option
                    selected={!f.ownSquad}
                    onClick={() => set({ ownSquad: false })}
                  >
                    Não
                  </Option>
                </Row>
              </>
            )}
          </div>
        )}
      </div>
      <div className="flex shrink-0 justify-end gap-2 border-t border-cm-dlgline p-3">
        <button
          className="h-9 w-32 rounded-[4px] border border-white/20 bg-black/30 font-semibold text-white hover:bg-black/50"
          onClick={onCancel}
        >
          Cancelar
        </button>
        <button
          className="h-9 w-32 rounded-[4px] bg-cm-dlgborder font-bold text-cm-onmint hover:bg-cm-mint"
          onClick={() => onOk(f)}
        >
          Ok
        </button>
      </div>
    </div>
  )
}
