import { useEffect, useMemo, useRef, useState } from "react"
import type { ReactNode } from "react"
import { api } from "../api"
import type { ClubListItem } from "../bindings/ClubListItem"
import type { ClubProfile } from "../bindings/ClubProfile"
import type { ClubTactics } from "../bindings/ClubTactics"
import type { Fixture } from "../bindings/Fixture"
import type { ReserveLineup } from "../bindings/ReserveLineup"
import type { SquadAttributes } from "../bindings/SquadAttributes"
import type { SquadMore } from "../bindings/SquadMore"
import type { SquadPlayer } from "../bindings/SquadPlayer"
import CascadeMenu, { type MenuNode } from "../components/CascadeMenu"
import { ClubFixtureList, MatchInfo } from "../components/Fixtures"
import FinancesPanel, { type FinanceView } from "../components/FinancesPanel"
import SquadTable, { HeaderCell, InfBadge, type HeaderProps } from "../components/SquadTable"
import { headerColumn } from "../squadColumns"
import TransfersPanel from "../components/TransfersPanel"
import {
  BottomBar,
  Empty,
  ErrorBox,
  Link,
  PanelTitle,
  PurpleMenu,
  ScreenHeader,
  Tabs,
  type MenuOption,
} from "../components/Screen"
import { money, shortListName } from "../format"
import { useGame } from "../game"
import { useNav } from "../nav"
import {
  BENCH,
  selectionOf,
  spotOf,
  swap,
  useDragSwap,
  type Spot,
} from "../selection"
import {
  FILTER_OPTIONS,
  loadFilter,
  saveFilter,
  VIEW_TITLE,
  ATTRIBUTE_GROUPS,
  MORE_COMPETITIONS,
  formAverage,
  moraleRank,
  sortByColumn,
  sortLabel,
  type AttributeGroup,
  type HeaderSort,
  nameClass,
  passes,
  squadName,
  type Filter,
  type SquadView,
} from "../squad"

const TABS = [
  { key: "squad", label: "Plantel" },
  { key: "transfers", label: "Transferências" },
  { key: "next", label: "Próximo Jogo" },
  { key: "fixtures", label: "Calendário" },
  { key: "info", label: "Finanças & Info" },
]

type View = SquadView | "staff"
type Criterion =
  | "name"
  | "position"
  | "number"
  | "form"
  | "morale"
  | "condition"
  | "nation"
  | "age"
  | "caps"
  | "intlGoals"
  | "wage"
  | "contract"
  | "value"
  | "goals"
  | "conceded"
  | "assists"
  | "rating"

// O Vêr do CM
const VIEWS: MenuOption<View>[] = [
  { key: "traditional", label: "Tradicional" },
  { key: "contract", label: "Contrato" },
  { key: "selection", label: "Selecção" },
  { key: "stats", label: "Estatísticas" },
  { key: "more", label: "Mais Dados" },
  { key: "attributes", label: "Atributos" },
  { key: "others", label: "Outros" },
  "separator",
  { key: "penalties", label: "Marcadores Penalties" },
  "separator",
  { key: "staff", label: "Staff" },
]

// O Critério do CM (a ordem da lista)
const CRITERIA: MenuOption<Criterion>[] = [
  { key: "name", label: "Nome" },
  { key: "position", label: "Posição" },
  { key: "number", label: "Numeração" },
  { key: "form", label: "Forma" },
  { key: "morale", label: "Moral" },
  { key: "condition", label: "Condição" },
  { key: "nation", label: "Nacionalidade" },
  { key: "age", label: "Idade" },
  { key: "caps", label: "Internacion." },
  { key: "intlGoals", label: "Golos Intern." },
  { key: "wage", label: "Salário Base" },
  { key: "contract", label: "Termo Contrato" },
  { key: "value", label: "Valor" },
  "separator",
  { key: "goals", label: "Golos" },
  { key: "conceded", label: "Golos Sofridos" },
  { key: "assists", label: "Assistências" },
  { key: "rating", label: "Class. Média" },
]
const criterionLabel = (c: Criterion) =>
  CRITERIA.find(
    (o): o is { key: Criterion; label: string } =>
      o !== "separator" && o.key === c,
  )?.label ?? ""

/** Uma linha do plantel: [número] [lugar/Int] Nome ... posição */
type Entry = {
  id: number
  number: ReactNode
  badge: ReactNode
  name: string
  detail: string
  spot?: Spot
  player?: SquadPlayer
}

type Bind = { "data-spot": string; onMouseDown: (e: React.MouseEvent) => void }

/** "D D" -> "DD", "M C" -> "MC": a sigla curta da função, como nas casas do plantel do CM. */
const shortRole = (role: string) => role.replace(" ", "")

const byNullsLast = (a: number | null, b: number | null) =>
  (a ?? 999) - (b ?? 999)

function SquadGrid({
  entries,
  selected,
  onSelect,
  onOpen,
  bind,
  onToggle,
  header,
  numbers,
}: {
  entries: Entry[]
  selected: number
  onSelect: (i: number) => void
  onOpen: (e: Entry) => void
  /** Só no clube do treinador: arrastar para mudar o lugar na equipa. */
  bind?: (spot: Spot, label: string) => Bind
  /** Só no clube do treinador: clicar no número ou na casa põe/tira da equipa. */
  onToggle?: (e: Entry) => void
  /** Os cabeçalhos que ordenam (fora do Staff). */
  header?: HeaderProps
  /** A coluna Nº (com o Critério Numeração ou Idade); o CM não a mostra. */
  numbers?: boolean
}) {
  const row = (e: Entry, i: number) => {
    const drag = bind && e.spot ? bind(e.spot, e.name) : undefined
    const toggle = onToggle && (() => onToggle(e))
    const stop = onToggle && ((ev: React.MouseEvent) => ev.stopPropagation())
    return (
      <div
        key={e.id}
        data-row={i}
        data-person={e.id}
        data-spot={drag?.["data-spot"]}
        onMouseDown={drag?.onMouseDown}
        onClick={() => onSelect(i)}
        onDoubleClick={() => onOpen(e)}
        className={`flex h-6 items-center gap-1.5 px-2 ${i === selected ? "!bg-cm-sel" : ""} ${drag ? "cursor-grab" : ""}`}
      >
        {numbers && (
          <span
            onMouseDown={stop}
            onClick={toggle}
            className={`w-8 shrink-0 text-center font-mono text-[13px] text-[#c8c8d0] ${onToggle ? "cursor-pointer" : ""}`}
          >
            {e.number}
          </span>
        )}
        {/* A casa na equipa e a sigla Inf em caixas escuras, vazias quando não há, como no CM */}
        <span
          onMouseDown={stop}
          onClick={toggle}
          title={onToggle ? "Clique para pôr/tirar da equipa" : undefined}
          className={`w-10 shrink-0 text-center font-mono text-[12px] 
            font-bold ${e.badge ? "cm-chip" : "h-[18px] rounded bg-black/50"} ${onToggle ? "cursor-pointer" : ""}`}
        >
          {e.badge}
        </span>
        {e.player && (
          <span className="flex h-[18px] w-10 shrink-0 items-center justify-center rounded bg-black/50 font-mono text-[12px]">
            {e.player.info && <InfBadge info={e.player.info} />}
          </span>
        )}
        <span
          className={`min-w-0 flex-1 truncate text-[16px] font-semibold ${e.player ? nameClass(e.player) : "text-white"}`}
          title={
            e.player?.ghost
              ? "Jogador de enchimento"
              : e.player?.loaned
                ? "Emprestado"
                : e.player?.free
                  ? "Sem contrato"
                  : undefined
          }
        >
          <Link onClick={() => onOpen(e)}>{e.name}</Link>
        </span>
        <span
          className={`w-28 shrink-0 truncate text-left font-mono text-[12.5px] ${i === selected ? "text-white" : "text-cm-yellow"}`}
        >
          {e.detail}
        </span>
      </div>
    )
  }
  const head = (
    <div className="flex h-7 shrink-0 items-center gap-1.5 border-b border-cm-line bg-black/30 px-2 font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-[#86869a]">
      {numbers && <HeaderCell label="Nº" className="w-8 text-center" header={header} />}
      <span className="w-10 text-center">Esc</span>
      {entries[0]?.player && <span className="w-10 text-center">Inf</span>}
      <HeaderCell label="Jogador" className="flex-1" header={header} />
      <HeaderCell label="Posição" className="w-28 text-left" header={header} />
    </div>
  )
  const column = (side: 0 | 1) => (
    <div className="min-w-0 flex-1 overflow-hidden rounded-md bg-cm-surface">
      {head}
      <div className="cm-zebra">
        {entries
          .map((e, i) => (i % 2 === side ? row(e, i) : null))
          .filter(Boolean)}
      </div>
    </div>
  )
  return (
    <div className="flex items-start gap-2">
      {column(0)}
      {column(1)}
    </div>
  )
}

/** Finanças pela palavra, como o CM (o banco guarda o saldo em euros). */
function financesLabel(cash: number | null) {
  if (cash === null) return "-"
  if (cash >= 50_000_000) return "Rico"
  if (cash >= 10_000_000) return "Seguro"
  if (cash >= 1_000_000) return "Razoável"
  if (cash >= 0) return "Apertado"
  return "Em dívida"
}

const INFO_VIEWS: { key: "info" | FinanceView; label: string }[] = [
  { key: "info", label: "Informação Geral" },
  { key: "finances", label: "Finanças" },
  { key: "income", label: "Receitas" },
  { key: "expenses", label: "Despesas" },
]

/** "Finanças & Info" do CM: o Ver ▾ escolhe a Informação Geral (dados do clube e o staff) ou
 * as Finanças, Receitas e Despesas. */
function Info({ c, mine }: { c: ClubProfile; mine: boolean }) {
  const [view, setView] = useState<"info" | FinanceView>("info")
  const menu = (
    <div className="px-2 pb-1 pt-2">
      <PurpleMenu label="Ver" options={INFO_VIEWS} value={view} onChange={setView} />
    </div>
  )
  if (view !== "info")
    return (
      <div className="min-h-0 flex-1 overflow-auto pb-3">
        {menu}
        <FinancesPanel clubId={c.club.id} view={view} mine={mine} />
      </div>
    )
  return <GeneralInfo c={c} menu={menu} />
}

function GeneralInfo({ c, menu }: { c: ClubProfile; menu: ReactNode }) {
  const nav = useNav()
  const row = (label: string, value: ReactNode) => (
    <div key={label} className="flex min-h-6 items-baseline gap-4 px-3 py-1">
      <span className="w-56 shrink-0 text-[15px] font-semibold text-white">
        {label}
      </span>
      <span className="flex-1 text-[14px] text-cm-yellow">{value}</span>
    </div>
  )
  const competition = c.division?.name
  return (
    <div className="min-h-0 flex-1 overflow-auto pb-3">
      {menu}
      <PanelTitle title="Informação Geral" />
      <div className="cm-zebra">
        {row(
          "País",
          c.nationId !== null && c.nation ? (
            <Link onClick={() => nav.go({ kind: "nation", id: c.nationId! })}>
              {c.nation}
            </Link>
          ) : (
            "-"
          ),
        )}
        {row(
          "Estatuto",
          `${c.status ?? "-"}${competition ? ` (${competition})` : ""}`,
        )}
        {row("Finanças", `${financesLabel(c.cash)} · saldo ${money(c.cash)}`)}
        {row(
          "Estádio",
          [c.stadium, c.stadiumCity].filter(Boolean).join(", ") || "-",
        )}
        {row(
          "Capacidade",
          c.stadiumCapacity
            ? `${c.stadiumCapacity}${c.stadiumSeated === c.stadiumCapacity ? " - todos lugares sentados" : c.stadiumSeated ? ` - (${c.stadiumSeated} lugares sentados)` : ""}${c.undersoilHeating ? " com aquecimento subterrâneo" : ""}${c.retractableRoof ? " e telhado retráctil" : ""}`
            : "-",
        )}
        {c.stadium && c.ownsStadium && row("Propriedade", "Estádio do clube")}
        {c.attendance &&
          row(
            "Assistência",
            `média ${c.attendance[0]} (mín. ${c.attendance[1]}, máx. ${c.attendance[2]})`,
          )}
        {c.matchDay && row("Dia de Jogo", c.matchDay)}
        {c.facilities && row("Condições Trabalho", c.facilities)}
        {row("Reputação", c.reputation ?? "-")}
        {c.reserveDivision &&
          row(
            "Estadual",
            <Link
              onClick={() =>
                nav.go({ kind: "competition", id: c.reserveDivision!.id })
              }
            >
              {c.reserveDivision.name}
            </Link>,
          )}
        {row(
          "Rivais",
          c.rivals.length
            ? c.rivals.map((r, i) => (
                <span key={r.id}>
                  {i > 0 && ", "}
                  <Link onClick={() => nav.go({ kind: "club", id: r.id })}>
                    {r.shortName}
                  </Link>
                </span>
              ))
            : "-",
        )}
      </div>
      <PanelTitle title="Staff" />
      {/* Uma linha por função, com os nomes (ligações para o perfil) */}
      <div className="cm-zebra">
        {[...new Set(c.staff.map((s) => s.job ?? "-"))].map((job) =>
          row(
            job,
            c.staff
              .filter((s) => (s.job ?? "-") === job)
              .map((s, i) => (
                <span key={s.id}>
                  {i > 0 && ", "}
                  <Link onClick={() => nav.go({ kind: "person", id: s.id })}>
                    {/* "Sobrenome, Nome" -> "Nome Sobrenome", como na lista do CM */}
                    {s.listName.includes(", ")
                      ? s.listName.split(", ").reverse().join(" ")
                      : s.listName}
                  </Link>
                </span>
              )),
          ),
        )}
      </div>
    </div>
  )
}

export default function ClubPage({
  id,
  reserves,
}: {
  id: number
  reserves?: boolean
}) {
  const nav = useNav()
  const game = useGame()
  const [club, setClub] = useState<ClubProfile | null>(null)
  const [fixtures, setFixtures] = useState<Fixture[]>([])
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState("squad")
  const [view, setView] = useState<View>("traditional")
  const [filter, setFilterState] = useState<Filter>(loadFilter)
  const setFilter = (update: (f: Filter) => Filter) =>
    setFilterState((f) => {
      const next = update(f)
      saveFilter(next)
      return next
    })
  const [criterion, setCriterion] = useState<Criterion>("position")
  const [headerSort, setHeaderSort] = useState<HeaderSort | null>(null)
  // Mais Dados (por competição) e Atributos (por grupo): o que vem do core para essas vistas
  const [moreKind, setMoreKind] = useState("")
  const [group, setGroup] = useState<AttributeGroup>("physical")
  const [more, setMore] = useState<Map<number, SquadMore>>(new Map())
  const [attributes, setAttributes] = useState<
    Map<number, SquadAttributes["attributes"]>
  >(new Map())
  const [selected, setSelected] = useState(0)
  const [tactics, setTactics] = useState<ClubTactics | null>(null)
  const gridRef = useRef<HTMLDivElement>(null)
  const mine = game.mode === "game" && game.info?.club.id === id
  // O menu do clube (▸ na faixa) e os clubes da divisão, pedidos ao abri-lo
  const [clubMenu, setClubMenu] = useState<{ x: number; y: number } | null>(
    null,
  )
  const [divisionClubs, setDivisionClubs] = useState<ClubListItem[] | null>(
    null,
  )

  useEffect(() => {
    api.getClub(id).then(setClub, (e) => setError(String(e)))
  }, [id])

  // Jogos do clube e a tabela da competição, recarregados a cada dia do jogo
  const date = game.info?.date
  useEffect(() => {
    if (!date) return
    api.clubFixtures(id).then(
      (fs) => {
        setFixtures(fs)
      },
      () => {},
    )
  }, [id, date])
  // Nas reservas do treinador, o onze do próximo jogo delas: clicar no número ou na casa põe
  // ou tira da equipa (o adjunto completa o resto)
  const [reserveLineup, setReserveLineup] = useState<ReserveLineup | null>(null)
  useEffect(() => {
    if (!date || !reserves || !mine) return
    api.reserveLineup().then(setReserveLineup, () => setReserveLineup(null))
  }, [date, reserves, mine])
  const setPicks = async (picks: number[]) => {
    try {
      setReserveLineup(await api.setReservePicks(picks))
      setError(null)
    } catch (err) {
      setError(String(err))
    }
  }
  const toggleReserve = (e: Entry) => {
    const picks = reserveLineup?.picks ?? []
    void setPicks(picks.includes(e.id) ? picks.filter((p) => p !== e.id) : [...picks, e.id])
  }
  // Nas reservas, o Calendário é o dos jogos delas (dois dias antes de cada jogo do campeonato)
  const [reserveFixtures, setReserveFixtures] = useState<Fixture[]>([])
  useEffect(() => {
    if (!date || !reserves) return
    api.reserveFixtures(id).then(setReserveFixtures, () => setReserveFixtures([]))
  }, [id, date, reserves])
  // No clube do treinador, a escolha da equipa (para as casas GR, DD... SB1 do plantel)
  useEffect(() => {
    if (mine) api.getClubTactics(id).then(setTactics, () => {})
  }, [id, mine, date])

  const drag = useDragSwap({
    enabled: mine && !!tactics,
    onClick: () => {},
    onDrop: async (from, to) => {
      if (!tactics) return
      try {
        setTactics(await api.setTactics(swap(selectionOf(tactics), from, to)))
        const info = await api.gameInfo()
        if (info) game.update(info)
      } catch (e) {
        setError(String(e))
      }
    },
  })

  // "Limpar Equipa", como no CM: tira todos da equipa (antes do jogo o adjunto preenche o que ficar vazio)
  const clearTeam = async () => {
    if (!tactics) return
    try {
      const sel = selectionOf(tactics)
      setTactics(
        await api.setTactics({
          ...sel,
          auto: false,
          starters: sel.starters.map(() => null),
          bench: [],
        }),
      )
      const info = await api.gameInfo()
      if (info) game.update(info)
      setError(null)
    } catch (err) {
      setError(String(err))
    }
  }

  const toggle = async (e: Entry) => {
    if (!tactics) return
    try {
      const next = await api.togglePlayer(selectionOf(tactics), e.id)
      setTactics(await api.setTactics(selectionOf(next)))
      const info = await api.gameInfo()
      if (info) game.update(info)
      setError(null)
    } catch (err) {
      setError(String(err))
    }
  }

  const nextMatch = fixtures.find((f) => f.homeGoals === null)
  const lastMatch = fixtures.filter((f) => f.homeGoals !== null).at(-1)

  // Ordenar ao clicar num cabeçalho, como no CM (a seta diz o sentido)
  const columnOf = (label: string) =>
    headerColumn(view, { more, attributes, group }, label)
  const clickHeader = (label: string) => {
    if (!columnOf(label)?.sort) return
    setHeaderSort((s) => ({ label, reverse: s?.label === label && !s.reverse }))
    setSelected(0)
  }
  const headerText = (label: string) =>
    sortLabel(label, headerSort, columnOf(label))

  // Os jogadores da vista: plantel principal ou reservas (com os séniores, se pedido), filtrados e ordenados
  const players: SquadPlayer[] = useMemo(() => {
    if (!club) return []
    const list = club.players.filter(
      (p) =>
        (reserves ? p.reserve || filter.seniors : !p.reserve || filter.reserves) &&
        passes(p, filter),
    )
    const name = (a: { listName: string }, b: { listName: string }) =>
      a.listName.localeCompare(b.listName)
    // Do maior para o menor nos números do jogo; os sem valor no fim
    const desc =
      (f: (p: SquadPlayer) => number | null | undefined) =>
      (a: SquadPlayer, b: SquadPlayer) =>
        byNullsLast(f(b) ?? null, f(a) ?? null) || name(a, b)
    const asc =
      (f: (p: SquadPlayer) => number | string | null | undefined) =>
      (a: SquadPlayer, b: SquadPlayer) => {
        const x = f(a) ?? null
        const y = f(b) ?? null
        if (x === null || y === null)
          return x === y ? name(a, b) : x === null ? 1 : -1
        return (
          (typeof x === "string"
            ? x.localeCompare(String(y))
            : x - Number(y)) || name(a, b)
        )
      }
    const sorters: Record<
      Criterion,
      (a: SquadPlayer, b: SquadPlayer) => number
    > = {
      name,
      position: (a, b) => a.positionOrder - b.positionOrder || name(a, b),
      number: asc((p) => p.squadNumber),
      form: desc(formAverage),
      morale: asc(moraleRank),
      condition: desc((p) => p.condition),
      nation: asc((p) => p.nation),
      age: asc((p) => p.age),
      caps: desc((p) => p.caps),
      intlGoals: desc((p) => p.intlGoals),
      wage: desc((p) => p.wage),
      contract: asc((p) => p.contractExpires),
      value: desc((p) => p.value),
      goals: desc((p) => p.goals),
      conceded: desc((p) => more.get(p.id)?.conceded),
      assists: desc((p) => p.assists),
      rating: desc((p) => p.rating),
    }
    list.sort(sorters[criterion])
    if (view === "penalties")
      list.sort((a, b) => byNullsLast(a.penaltyOrder, b.penaltyOrder))
    // Por cima do Critério, o cabeçalho clicado (outra vez: ao contrário)
    const col =
      headerSort &&
      headerColumn(view, { more, attributes, group }, headerSort.label)
    if (col) sortByColumn(list, col, headerSort.reverse)
    return list
  }, [club, reserves, filter, criterion, view, more, attributes, group, headerSort])

  const needMore = view === "more" || criterion === "conceded"
  useEffect(() => {
    if (!needMore || game.mode !== "game") return
    api.squadMore(id, moreKind).then(
      (rows) => setMore(new Map(rows.map((r) => [r.person, r]))),
      () => setMore(new Map()),
    )
  }, [needMore, id, moreKind, game.mode, game.info?.date])
  useEffect(() => {
    if (view !== "attributes" || game.mode !== "game") return
    api.squadAttributes(id).then(
      (rows) =>
        setAttributes(new Map(rows.map((r) => [r.person, r.attributes]))),
      () => setAttributes(new Map()),
    )
  }, [view, id, game.mode, game.info?.date])

  const sel = tactics && selectionOf(tactics)
  // Casa na equipa (GR, DD... e SB1–SB7 nos suplentes); nas reservas, a do onze delas
  const placeOf = (pid: number) => {
    if (reserves) {
      const r = reserveLineup?.starters.find((s) => s.person === pid)
      return r ? shortRole(r.role) : null
    }
    const spot = sel ? spotOf(sel, pid) : undefined
    return spot?.kind === "starter"
      ? shortRole(tactics!.starters[spot.index].role)
      : spot?.kind === "bench"
        ? `SB${spot.index + 1}`
        : null
  }

  const entries: Entry[] = !club
    ? []
    : view === "staff"
      ? club.staff.map((s) => ({
          id: s.id,
          number: "",
          badge: "",
          name: shortListName(s.listName),
          detail: s.job ?? "",
        }))
      : players.map((p) => ({
          id: p.id,
          spot: sel ? spotOf(sel, p.id) : undefined,
          number: criterion === "age" ? (p.age ?? "") : (p.squadNumber ?? ""),
          badge: placeOf(p.id) ?? "",
          name: squadName(p),
          detail: p.position,
          player: p,
        }))

  const table = view !== "traditional" && view !== "staff"
  // Casas da equipa, Limpar Equipa e pôr/tirar: na vista Tradicional (nas reservas, o onze delas)
  const teamView = !reserves && view === "traditional"
  const reserveView = reserves && mine && view === "traditional" && !!reserveLineup
  const rowCount = table ? players.length : entries.length

  const open = (e: { id: number } | undefined) =>
    e && nav.go({ kind: "person", id: e.id })

  // Teclado: setas andam na grade de 2 colunas, Enter abre, Esc/Backspace voltam
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey) return
      // Nas tabelas do Vêr, uma coluna só
      const steps: Record<string, number> = table
        ? { ArrowUp: -1, ArrowDown: 1 }
        : { ArrowUp: -2, ArrowDown: 2, ArrowLeft: -1, ArrowRight: 1 }
      const step = steps[e.key]
      if (step !== undefined && tab === "squad") {
        e.preventDefault()
        setSelected((s) => {
          const next = Math.max(0, Math.min(rowCount - 1, s + step))
          gridRef.current
            ?.querySelector(`[data-row="${next}"]`)
            ?.scrollIntoView({ block: "nearest" })
          return next
        })
      } else if (e.key === "Enter" && tab === "squad") {
        open(table ? players[selected] : entries[selected])
      } else if (e.key === "Escape" || e.key === "Backspace") {
        nav.back()
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  })

  const division = club?.division

  // Lugar de treinador vago: "Concorrer ao Lugar", como no CM
  const [vacancy, setVacancy] = useState<{
    club: number
    text: string | null
  } | null>(null)
  const nationId = club?.nationId ?? null
  const ownClub = club?.club.id === game.info?.club.id
  useEffect(() => {
    if (game.mode !== "game" || ownClub || nationId === null) return
    api.jobsView(nationId).then(
      (j) => {
        const row = j.rows.find((r) => r.club.id === id)
        setVacancy(
          row?.vacant && !row.applied ? { club: id, text: null } : null,
        )
      },
      () => setVacancy(null),
    )
  }, [game.mode, ownClub, nationId, id])
  const apply = async () => {
    try {
      const text = await api.applyJob(id)
      setVacancy({ club: id, text })
    } catch (e) {
      setVacancy({ club: id, text: String(e) })
    }
  }

  // Menu do clube, como o ▸ da faixa do CM: reservas/principal, os clubes ligados, a divisão e a nação
  const openClubMenu = (e: React.MouseEvent) => {
    const r = e.currentTarget.getBoundingClientRect()
    setClubMenu(clubMenu ? null : { x: r.left, y: r.bottom + 2 })
    if (division && !divisionClubs)
      api.getCompetition(division.id).then(
        (c) => setDivisionClubs(c.clubs),
        () => setDivisionClubs([]),
      )
  }
  const clubMenuItems = (): MenuNode[] => {
    if (!club) return []
    const items: MenuNode[] = []
    if (mine && !reserves)
      items.push({
        label: "Plantel de Reservas",
        onSelect: () => nav.go({ kind: "club", id, reserves: true }),
      })
    if (reserves)
      items.push({
        label: "Plantel Principal",
        onSelect: () => nav.go({ kind: "club", id }),
      })
    // As equipas B/II (Bayern Munich II...) são outros clubes: ficam na lista da divisão delas
    const others = (divisionClubs ?? [])
      .filter((c) => c.club.id !== id)
      .sort((a, b) => a.club.shortName.localeCompare(b.club.shortName))
      .map((c) => ({
        label: c.club.shortName,
        onSelect: () => nav.go({ kind: "club", id: c.club.id }),
      }))
    if (division) {
      if (items.length) items.push("separator")
      items.push({
        label: division.name,
        children: others.length ? others : undefined,
        onSelect: () => nav.go({ kind: "competition", id: division.id }),
      })
    }
    if (club.nationId !== null && club.nation)
      items.push({
        label: club.nation,
        onSelect: () => nav.go({ kind: "nation", id: club.nationId! }),
      })
    return items
  }

  const filterOptions: MenuOption<string>[] = reserves
    ? [
        ...FILTER_OPTIONS,
        "separator",
        { key: "seniors", label: "Incluir Séniores" },
      ]
    : mine
      ? [...FILTER_OPTIONS, "separator", { key: "reserves", label: "Incluir Reservas" }]
      : FILTER_OPTIONS
  const filterChecked = (key: string) => {
    if (key === "seniors") return filter.seniors
    if (key === "reserves") return filter.reserves
    const [group, value] = key.split(":")
    return group === "line"
      ? filter.line === value
      : group === "side"
        ? filter.side === value
        : filter.avail === value
  }
  const changeFilter = (key: string) => {
    const [group, value] = key.split(":")
    setFilter((f) =>
      key === "seniors"
        ? { ...f, seniors: !f.seniors }
        : key === "reserves"
          ? { ...f, reserves: !f.reserves }
          : group === "line"
          ? { ...f, line: value }
          : group === "side"
            ? { ...f, side: value }
            : // Disponíveis / Indisponíveis: um clique marca, outro desmarca
              {
                ...f,
                avail: f.avail === value ? "" : (value as Filter["avail"]),
              },
    )
    setSelected(0)
  }
  const filtered = filter.line || filter.side || filter.avail

  return (
    <div className="relative flex h-full flex-col">
      <ScreenHeader
        club={club?.club}
        eyebrow={
          club && (
            <>
              <span className="text-cm-orange">
                {club.division?.name ?? club.status ?? ""}
              </span>
              {reserves && (
                <span className="rounded bg-[#0f3a2a] px-1.5 text-[#4edea3]">
                  Reservas
                </span>
              )}
            </>
          )
        }
        title={
          club ? (reserves ? `Reservas ${club.club.name}` : club.club.name) : ""
        }
        right={
          <>
            {club?.stadium && (
              <span className="hidden rounded-md bg-black/30 px-3 py-1.5 font-mono text-[12px] text-[#86869a] xl:inline">
                Estádio: <b className="text-white">{club.stadium}</b>
                {club.stadiumCapacity ? (
                  <>
                    {" "}
                    · Capacidade:{" "}
                    <b className="text-white">
                      {club.stadiumCapacity.toLocaleString("pt-BR")}
                    </b>
                  </>
                ) : null}
              </span>
            )}
            {vacancy?.club === id &&
              (vacancy.text === null ? (
                <button
                  className="h-8 rounded bg-cm-orange px-4 font-title text-[15px] font-bold text-black hover:brightness-110"
                  onClick={() => void apply()}
                >
                  Concorrer ao Lugar
                </button>
              ) : (
                <span className="rounded bg-black/60 px-2 py-1 text-[13px] text-cm-yellow">
                  {vacancy.text}
                </span>
              ))}
            {club && (
              <button
                data-menu-anchor
                className="cm-purple flex h-8 w-9 items-center justify-center text-[13px]"
                title="Menu do clube"
                aria-expanded={!!clubMenu}
                onClick={openClubMenu}
              >
                ▸
              </button>
            )}
          </>
        }
      />
      <Tabs
        tabs={TABS.map((t) =>
          t.key === "next" || t.key === "fixtures"
            ? { ...t, disabled: fixtures.length === 0 }
            : t.key === "transfers"
              ? { ...t, disabled: game.mode !== "game" }
              : t,
        )}
        active={tab}
        onChange={setTab}
      />

      {tab === "squad" && (
        <div className="mx-2 mt-2 flex shrink-0 items-center gap-1.5 rounded-md bg-[#3d1a73] px-2 py-1.5">
          <PurpleMenu
            label="Vêr"
            options={VIEWS}
            value={view}
            onChange={(v) => {
              setView(v)
              setHeaderSort(null)
              setSelected(0)
            }}
          />
          {view === "more" ? (
            <PurpleMenu
              label="Competições"
              options={MORE_COMPETITIONS}
              value={moreKind}
              onChange={setMoreKind}
            />
          ) : view === "attributes" ? (
            <PurpleMenu
              label="Atributos"
              options={ATTRIBUTE_GROUPS}
              value={group}
              onChange={setGroup}
            />
          ) : (
            <PurpleMenu
              label="Critério"
              options={CRITERIA}
              value={criterion}
              disabled={view === "staff"}
              onChange={(c) => {
                setCriterion(c)
                setHeaderSort(null)
                setSelected(0)
              }}
            />
          )}
          <div className="flex-1" />
          {mine && (
            <button
              className="cm-purple h-6 w-40"
              disabled={reserves ? !reserveView || !reserveLineup?.picks.length : !tactics || !teamView}
              onClick={() => void (reserves ? setPicks([]) : clearTeam())}
            >
              Limpar Equipa
            </button>
          )}
          {mine && (
            <button
              className="cm-purple h-6 w-40"
              disabled={!game.info?.numbersOpen}
              title={
                game.info?.numbersOpen
                  ? undefined
                  : "Os números só se mudam na pré-época"
              }
              onClick={() => nav.go({ kind: "squadNumbers" })}
            >
              Números
            </button>
          )}
          <PurpleMenu
            label={filtered || filter.seniors ? "Filtrar ✓" : "Filtrar"}
            options={filterOptions}
            checked={filterChecked}
            onChange={changeFilter}
            disabled={view === "staff"}
            align="right"
          />
        </div>
      )}

      <div className="mx-2 mt-2 flex min-h-0 flex-1 flex-col">
        <ErrorBox className="mb-2">{error}</ErrorBox>
        {club && tab === "squad" && (
          <>
            {/* Casas da equipa: arraste um jogador para uma casa (ou para outro jogador) para mudar o lugar */}
            <div className="mb-2 flex shrink-0 flex-wrap items-center gap-0.5 rounded-md bg-cm-surface px-3 py-1.5">
              <span className="mr-2 font-mono text-[12px] font-bold uppercase tracking-[0.12em] text-cm-orange">
                {view === "staff"
                  ? "Staff"
                  : view === "traditional"
                    ? criterionLabel(criterion)
                    : view === "more"
                      ? `Estatísticas - ${MORE_COMPETITIONS.find((c) => c.key === moreKind)?.label}`
                      : view === "attributes"
                        ? ATTRIBUTE_GROUPS.find((g) => g.key === group)?.title
                        : VIEW_TITLE[view]}
              </span>
              {mine && tactics && teamView && (
                <>
                  {tactics.starters.map((s, i) => {
                    const b = drag.bind(
                      { kind: "starter", index: i },
                      shortRole(s.role),
                    )
                    return (
                      <span
                        key={`s${i}`}
                        {...b}
                        title={s.player?.listName ?? "vazio"}
                        className="cm-chip min-w-[38px] cursor-grab px-1 text-center text-[12px]"
                      >
                        {shortRole(s.role)}
                      </span>
                    )
                  })}
                  {/* Banco fixo de 9, como no CM: arraste um jogador para uma casa SB vazia para o pôr no banco */}
                  {Array.from({ length: BENCH }, (_, i) => {
                    const p = tactics.bench[i]
                    const b = drag.bind(
                      { kind: "bench", index: i },
                      `SB${i + 1}`,
                    )
                    return (
                      <span
                        key={`b${i}`}
                        {...b}
                        title={p?.listName ?? "vazio"}
                        className={`min-w-[38px] cursor-grab px-1 text-center text-[12px] ${p ? "cm-chip" : "cm-chip !bg-[#2a1a45] !text-[#8a63c4]"}`}
                      >
                        SB{i + 1}
                      </span>
                    )
                  })}
                  <span className="ml-2 self-center text-[11px] text-[#86869a]">
                    Arraste para trocar · clique no número ou na casa para
                    pôr/tirar
                  </span>
                </>
              )}
              {reserveView && (
                <span className="ml-2 self-center text-[11px] text-[#86869a]">
                  {reserveLineup.picks.length
                    ? `Escolheu ${reserveLineup.picks.length} · o adjunto completa o onze`
                    : "O adjunto escolhe o onze"}{" "}
                  · clique no número ou na casa para pôr/tirar da equipa das reservas
                </span>
              )}
            </div>
            <div ref={gridRef} className="min-h-0 flex-1 overflow-auto">
              {table && players.length ? (
                <SquadTable
                  view={view}
                  extra={{ more, attributes, group }}
                  rows={players.map((p) => ({ p, place: placeOf(p.id) }))}
                  selected={selected}
                  onSelect={setSelected}
                  onOpen={(pid) => open({ id: pid })}
                  header={{ text: headerText, onClick: clickHeader }}
                />
              ) : !table && entries.length ? (
                <SquadGrid
                  entries={entries}
                  selected={selected}
                  onSelect={setSelected}
                  onOpen={open}
                  header={view === "staff" ? undefined : { text: headerText, onClick: clickHeader }}
                  numbers={criterion === "number" || criterion === "age" || headerSort?.label === "Nº"}
                  bind={mine && tactics && !reserves ? drag.bind : undefined}
                  onToggle={
                    reserveView
                      ? toggleReserve
                      : mine && tactics && teamView
                      ? (e) => void (e.player?.reserve ? undefined : toggle(e))
                      : undefined
                  }
                />
              ) : (
                <Empty>
                  {filtered
                    ? "Nenhum jogador com este filtro."
                    : "Ninguém registado."}
                </Empty>
              )}
            </div>
          </>
        )}
        {tab !== "squad" && (
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-md bg-cm-surface">
            {club && tab === "info" && (
              <Info c={club} mine={mine} />
            )}
            {tab === "next" && (
              <MatchInfo
                key="next"
                clubId={id}
                fixtures={fixtures}
                start={
                  nextMatch ? fixtures.indexOf(nextMatch) : fixtures.length
                }
                empty="Não há mais jogos marcados."
              />
            )}
            {tab === "last" && (
              <MatchInfo
                key="last"
                clubId={id}
                fixtures={fixtures}
                start={lastMatch ? fixtures.indexOf(lastMatch) : -1}
                empty="Ainda não jogou."
              />
            )}
            {tab === "transfers" && <TransfersPanel clubId={id} />}
            {tab === "fixtures" && (
              <ClubFixtureList clubId={id} fixtures={reserves ? reserveFixtures : fixtures} />
            )}
          </div>
        )}
      </div>

      {/* Atalhos inferiores do CM; o Treino só no clube do treinador */}
      <div className="mt-2">
        <Tabs
          tabs={[
            { key: "tactics", label: "Tácticas" },
            { key: "training", label: "Treino", disabled: !mine },
            { key: "last", label: "Último Jogo", disabled: !lastMatch },
            // Num clube de reservas ou juniores, o atalho leva à Equipa Principal
            club?.parent
              ? { key: "parent", label: "Equipa Principal" }
              : {
                  key: "division",
                  label: division?.shortName ?? "-",
                  disabled: !division,
                },
            { key: "history", label: "Historial" },
          ]}
          active={tab === "last" ? "last" : ""}
          onChange={(k) => {
            if (k === "last") setTab("last")
            if (k === "division" && division)
              nav.go({ kind: "competition", id: division.id })
            if (k === "parent" && club?.parent)
              nav.go({ kind: "club", id: club.parent.id })
            if (k === "tactics") nav.go({ kind: "tactics", id })
            if (k === "training") nav.go({ kind: "training" })
            if (k === "history") nav.go({ kind: "clubHistory", id })
          }}
        />
      </div>
      <BottomBar
        onLeft={nav.canBack ? nav.back : undefined}
        onRight={nav.canForward ? nav.forward : undefined}
      />
      {drag.ghost}
      {clubMenu && (
        <CascadeMenu
          x={clubMenu.x}
          y={clubMenu.y}
          items={clubMenuItems()}
          onClose={() => setClubMenu(null)}
        />
      )}
    </div>
  )
}
