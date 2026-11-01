import type { ReactNode } from "react"
import type { SquadAttributes } from "../bindings/SquadAttributes"
import type { SquadMore } from "../bindings/SquadMore"
import type { SquadPlayer } from "../bindings/SquadPlayer"
import { cmDate, money, wage } from "../format"
import { ATTRIBUTE_GROUPS, nameClass, squadName, type AttributeGroup, type SquadView } from "../squad"
import { Link } from "./Screen"

// As vistas em tabela do Vêr do plantel, como no CM: uma linha por jogador com Esc, Inf e
// Nome à esquerda e as colunas da vista (contrato, selecção, estatísticas, penáltis).

export type TableRow = { p: SquadPlayer; place: string | null }

type Column = {
  label: string
  width: string
  align?: "left" | "right" | "center"
  cell: (p: SquadPlayer) => ReactNode
}

const rating = (r: number | null) =>
  r === null ? "-" : r.toFixed(2).replace(".", ",")

/** "(3)" a seguir aos jogos: quantos foram a sair do banco. */
const apps = (p: SquadPlayer) =>
  p.apps ? `${p.apps - p.subs}${p.subs ? ` (${p.subs})` : ""}` : "-"

/** O que as vistas novas precisam do core: o Mais Dados e os atributos de cada jogador. */
export type TableExtra = {
  more?: Map<number, SquadMore>
  attributes?: Map<number, SquadAttributes["attributes"]>
  group?: AttributeGroup
}

const value = (p: SquadPlayer) => (p.free ? "Livre" : money(p.value))
const num = (v: number | undefined) => (v === undefined ? "-" : v)

/** As colunas de uma vista; as do Mais Dados e Atributos dependem do que veio do core. */
function columnsFor(view: Exclude<SquadView, "traditional">, extra: TableExtra): Column[] {
  if (view === "more") {
    const m = (p: SquadPlayer) => extra.more?.get(p.id)
    return [
      { label: "Sof", width: "w-12", align: "right", cell: (p) => num(m(p)?.conceded) },
      { label: "Pens", width: "w-12", align: "right", cell: () => "-" },
      { label: "Amr", width: "w-12", align: "right", cell: (p) => num(m(p)?.yellows) },
      { label: "Verm", width: "w-12", align: "right", cell: (p) => num(m(p)?.reds) },
      { label: "V", width: "w-12", align: "right", cell: (p) => num(m(p)?.wins) },
      { label: "D", width: "w-12", align: "right", cell: (p) => num(m(p)?.losses) },
      { label: "GlsE", width: "w-12", align: "right", cell: (p) => num(m(p)?.teamFor) },
      { label: "SofE", width: "w-12", align: "right", cell: (p) => num(m(p)?.teamAgainst) },
      { label: "Cla M", width: "w-16", align: "right", cell: (p) => rating(m(p)?.rating ?? null) },
      { label: "Valor", width: "w-24", align: "right", cell: value },
    ]
  }
  if (view === "attributes") {
    const group = ATTRIBUTE_GROUPS.find((g) => g.key === extra.group) ?? ATTRIBUTE_GROUPS[0]
    return [
      ...group.columns.map(([key, label]): Column => ({
        label,
        width: "w-11",
        align: "right",
        cell: (p) => extra.attributes?.get(p.id)?.[key] ?? "-",
      })),
      { label: "Valor", width: "w-24", align: "right", cell: value },
    ]
  }
  return COLUMNS[view]
}

const COLUMNS: Record<Exclude<SquadView, "traditional" | "more" | "attributes">, Column[]> = {
  contract: [
    { label: "Situação", width: "w-28", cell: (p) => p.role ?? "-" },
    {
      label: "Salário Base",
      width: "w-28",
      align: "right",
      cell: (p) => (p.free ? "-" : wage(p.wage) || "-"),
    },
    {
      label: "Termo Contrato",
      width: "w-32",
      align: "center",
      cell: (p) => (p.free ? "-" : cmDate(p.contractExpires)),
    },
    {
      label: "Cláusulas",
      width: "w-32",
      cell: (p) => (p.clauses.length ? p.clauses.join(", ") : "-"),
    },
    {
      label: "Valor",
      width: "w-28",
      align: "right",
      cell: (p) => (p.free ? "Livre" : money(p.value)),
    },
  ],
  selection: [
    { label: "Posição", width: "w-28", cell: (p) => p.position },
    {
      label: "Forma",
      width: "w-32",
      cell: (p) =>
        p.form.length ? p.form.map((f) => Math.round(f)).join(" ") : "-",
    },
    { label: "Moral", width: "w-28", cell: (p) => p.morale ?? "-" },
    {
      label: "Cond.",
      width: "w-16",
      align: "right",
      cell: (p) => (p.condition === null ? "-" : `${p.condition}%`),
    },
    { label: "Pres", width: "w-20", align: "right", cell: apps },
    {
      label: "Cla M",
      width: "w-16",
      align: "right",
      cell: (p) => rating(p.rating),
    },
    {
      label: "Valor",
      width: "w-28",
      align: "right",
      cell: (p) => (p.free ? "Livre" : money(p.value)),
    },
  ],
  stats: [
    { label: "Posição", width: "w-28", cell: (p) => p.position },
    { label: "Jogos", width: "w-20", align: "right", cell: apps },
    {
      label: "Golos",
      width: "w-16",
      align: "right",
      cell: (p) => p.goals || "-",
    },
    {
      label: "Assist",
      width: "w-16",
      align: "right",
      cell: (p) => p.assists || "-",
    },
    { label: "HJ", width: "w-12", align: "right", cell: (p) => p.mom || "-" },
    {
      label: "Cla M",
      width: "w-16",
      align: "right",
      cell: (p) => rating(p.rating),
    },
  ],
  others: [
    { label: "Nat.", width: "w-14", cell: (p) => p.nationCode ?? "-" },
    { label: "Idade", width: "w-14", align: "right", cell: (p) => p.age ?? "-" },
    { label: "Ints", width: "w-12", align: "right", cell: (p) => p.caps },
    { label: "Golos", width: "w-14", align: "right", cell: (p) => p.intlGoals },
    {
      label: "Forma",
      width: "w-28",
      cell: (p) => (p.form.length ? p.form.map((f) => Math.round(f)).join("-") : "-"),
    },
    { label: "Moral", width: "w-24", cell: (p) => p.morale ?? "-" },
    {
      label: "Cond.",
      width: "w-14",
      align: "right",
      cell: (p) => (p.condition === null ? "-" : `${p.condition}%`),
    },
    { label: "Valor", width: "w-24", align: "right", cell: value },
  ],
  penalties: [
    { label: "Posição", width: "w-28", cell: (p) => p.position },
    {
      label: "Ordem",
      width: "w-16",
      align: "right",
      cell: (p) => p.penaltyOrder ?? "-",
    },
  ],
}

const ALIGN = { left: "text-left", right: "text-right", center: "text-center" }

/** A sigla da coluna Inf, nas cores do CM (lesão laranja, castigo vermelho). */
export function InfBadge({ info }: { info: string | null }) {
  if (!info) return <span className="text-[#5a5a66]">-</span>
  const colour =
    info === "Les"
      ? "bg-[#b8560c] text-white"
      : info === "Sus"
        ? "bg-[#9c0000] text-white"
        : "bg-gray-900 text-cm-yellow border-gray-700 border"
  return <span className={`px-1 font-bold rounded ${colour}`}>{info}</span>
}

export default function SquadTable({
  view,
  rows,
  selected,
  onSelect,
  onOpen,
  extra = {},
}: {
  view: Exclude<SquadView, "traditional">
  rows: TableRow[]
  selected: number
  onSelect: (i: number) => void
  onOpen: (id: number) => void
  extra?: TableExtra
}) {
  const columns = columnsFor(view, extra)
  const head =
    "font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-[#86869a]"
  return (
    <div className="overflow-hidden rounded-md bg-cm-surface">
      <div
        className={`flex h-7 items-center gap-2 border-b border-cm-line bg-black/30 px-2 ${head}`}
      >
        <span className="w-10 text-center">Esc</span>
        <span className="w-10 text-center">Inf</span>
        <span className="min-w-28 flex-1">Nome</span>
        {columns.map((c) => (
          <span
            key={c.label}
            className={`${c.width} shrink-0 ${ALIGN[c.align ?? "left"]}`}
          >
            {c.label}
          </span>
        ))}
      </div>
      <div className="cm-zebra">
        {rows.map(({ p, place }, i) => (
          <div
            key={p.id}
            data-row={i}
            onClick={() => onSelect(i)}
            onDoubleClick={() => onOpen(p.id)}
            className={`flex h-6 items-center gap-2 px-2 ${i === selected ? "!bg-cm-sel" : ""}`}
          >
            <span className="w-10 shrink-0 text-center font-mono text-[12px] font-bold text-[#4edea3]">
              {place ?? <span className="text-[#5a5a66]">-</span>}
            </span>
            <span className="w-10 shrink-0 text-center font-mono text-[12px]">
              <InfBadge info={p.info} />
            </span>
            <span
              className={`min-w-28 flex-1 truncate text-[15px] font-semibold ${nameClass(p)}`}
            >
              <Link onClick={() => onOpen(p.id)}>{squadName(p)}</Link>
            </span>
            {columns.map((c) => (
              <span
                key={c.label}
                className={`${c.width} shrink-0 truncate font-mono text-[12.5px] ${ALIGN[c.align ?? "left"]} ${i === selected ? "text-white" : "text-cm-yellow"}`}
              >
                {c.cell(p)}
              </span>
            ))}
          </div>
        ))}
      </div>
    </div>
  )
}
