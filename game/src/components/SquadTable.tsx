import type { SquadPlayer } from "../bindings/SquadPlayer"
import { nameClass, squadName } from "../squad"
import { columnsFor, type TableExtra, type TableView } from "../squadColumns"
import { Link } from "./Screen"

// As vistas em tabela do Vêr do plantel, como no CM: uma linha por jogador com Esc, Inf e
// Nome à esquerda e as colunas da vista (contrato, selecção, estatísticas, penáltis).

export type TableRow = { p: SquadPlayer; place: string | null }

const ALIGN = { left: "text-left", right: "text-right", center: "text-center" }

/** Os cabeçalhos que ordenam: o texto (com a seta) e o clique. */
export type HeaderProps = { text: (label: string) => string; onClick: (label: string) => void }

/** Um cabeçalho de coluna; com `header`, clicável para ordenar por ele. */
export function HeaderCell({ label, className, header }: { label: string; className: string; header?: HeaderProps }) {
  if (!header) return <span className={className}>{label}</span>
  return (
    <span
      className={`${className} cursor-pointer select-none hover:text-white`}
      title="Clique para ordenar"
      onClick={() => header.onClick(label)}
    >
      {header.text(label)}
    </span>
  )
}

/** A sigla da coluna Inf, nas cores do CM (lesão laranja, castigo vermelho). */
export function InfBadge({ info }: { info: string | null }) {
  if (!info) return <span className="text-[#5a5a66]">-</span>
  const colour =
    info === "Les"
      ? "bg-[#b8560c] text-white"
      : info === "Sus" || info === "Fit"
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
  header,
}: {
  view: TableView
  rows: TableRow[]
  selected: number
  onSelect: (i: number) => void
  onOpen: (id: number) => void
  extra?: TableExtra
  header?: HeaderProps
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
        <HeaderCell label="Nome" className="min-w-28 flex-1" header={header} />
        {columns.map((c) => (
          <HeaderCell
            key={c.label}
            label={c.label}
            className={`${c.width} shrink-0 ${ALIGN[c.align ?? "left"]}`}
            header={c.sort ? header : undefined}
          />
        ))}
      </div>
      <div className="cm-zebra">
        {rows.map(({ p, place }, i) => (
          <div
            key={p.id}
            data-row={i}
            data-person={p.id}
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
