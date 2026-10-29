import { ChevronDown, Search } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { api } from '../api'
import type { MarketStatus } from '../bindings/MarketStatus'
import type { NationOption } from '../bindings/NationOption'
import type { PersonKind } from '../bindings/PersonKind'
import type { PersonSummary } from '../bindings/PersonSummary'
import type { SearchResult } from '../bindings/SearchResult'
import type { SearchSort } from '../bindings/SearchSort'
import ScoutPanel from '../components/ScoutPanel'
import { BottomBar, ErrorBox, Link, PurpleMenu, ScreenHeader, Tabs } from '../components/Screen'
import { money } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'
import FilterDialog from './FilterDialog'
import { EMPTY_FILTERS, describe, toQuery, type Filters } from './searchFilters'

const PAGE_SIZE = 100

const TABS = [
  { key: 'shortlist', label: 'Lista Preferencial' },
  { key: 'players', label: 'Pesquisar Jogador' },
  { key: 'staff', label: 'Pesquisa Staff' },
  { key: 'scout1', label: 'Olheiro #1' },
  { key: 'scout2', label: 'Olheiro #2' },
]
// As abas dos outros olheiros ficam em baixo, como no CM
const MORE_SCOUTS = [3, 4, 5, 6, 7].map((n) => ({ key: `scout${n}`, label: `Olheiro #${n}` }))

function useDebounced<T>(value: T, ms: number) {
  const [debounced, setDebounced] = useState(value)
  useEffect(() => {
    const t = setTimeout(() => setDebounced(value), ms)
    return () => clearTimeout(t)
  }, [value, ms])
  return debounced
}

// O "Vêr" da pesquisa: situação no mercado
type Status = 'all' | MarketStatus
const STATUSES: { key: Status; label: string }[] = [
  { key: 'all', label: 'Todos' },
  { key: 'transfer', label: 'Listados para transferência' },
  { key: 'loan', label: 'Listados para empréstimo' },
  { key: 'free', label: 'Sem contrato' },
]

function Row({
  p,
  index,
  selected,
  onSelect,
  onOpen,
  onOpenClub,
}: {
  p: PersonSummary
  index: number
  selected: boolean
  onSelect: () => void
  onOpen: () => void
  onOpenClub: (id: number) => void
}) {
  return (
    <tr
      className={`h-[30px] text-[14px] ${selected ? 'bg-cm-sel' : 'hover:bg-white/[0.05]'}`}
      onMouseDown={onSelect}
      onDoubleClick={onOpen}
      data-row={index}
    >
      <td className="truncate px-3 text-white">
        <Link onClick={onOpen}>{p.listName}</Link>
      </td>
      <td className={`truncate px-2 ${selected ? 'text-white' : 'text-cm-magenta'}`}>
        {p.clubId !== null && p.club ? <Link onClick={() => onOpenClub(p.clubId!)}>{p.club}</Link> : 'Sem clube'}
      </td>
      <td className={`truncate px-2 ${selected ? 'text-white' : 'text-cm-yellow'}`}>{p.nation ?? '-'}</td>
      <td className={`truncate px-2 ${selected ? 'text-white' : 'text-cm-orange'}`}>{p.position || p.job}</td>
      <td className="truncate px-1">
        {p.tags.map((t) => (
          <span key={t} className="mr-1 rounded-[3px] bg-cm-magenta/15 px-1 text-[11px] font-bold text-cm-magenta">
            {t}
          </span>
        ))}
      </td>
      <td className="px-2 text-center tabular-nums">{p.age ?? '-'}</td>
      <td className={`px-3 text-right font-semibold tabular-nums ${selected ? 'text-white' : 'text-[#4edea3]'}`}>{p.value === null ? '-' : money(p.value)}</td>
    </tr>
  )
}

/** Cabeçalho roxo de coluna; clicar ordena pela coluna (quando ela permite). */
function Th({ label, sort, current, onSort, className = '' }: { label: string; sort?: SearchSort; current: SearchSort; onSort: (s: SearchSort) => void; className?: string }) {
  const active = sort !== undefined && sort === current
  return (
    <th
      onClick={sort ? () => onSort(sort) : undefined}
      className={`bg-[#16171c] px-3 text-[11px] font-bold font-mono uppercase tracking-[0.12em] ${sort ? 'cursor-pointer hover:text-white' : ''} ${
        active ? 'text-cm-yellow' : 'text-[#86869a]'
      } ${className}`}
    >
      {label}
      {active && ' ▾'}
    </th>
  )
}

function PageButton({ label, onClick, disabled }: { label: string; onClick: () => void; disabled: boolean }) {
  return (
    <button onClick={onClick} disabled={disabled} className="rounded px-1.5 font-bold text-cm-yellow hover:bg-white/10 disabled:text-white/25 disabled:hover:bg-transparent">
      {label}
    </button>
  )
}

export default function SearchPage({ active }: { active: boolean }) {
  const nav = useNav()
  const game = useGame()
  // "Fundos": o orçamento de transferências, no canto da faixa como no CM
  const [funds, setFunds] = useState<number | null>(null)
  useEffect(() => {
    if (game.mode === 'game') api.boardView().then((b) => setFunds(b.transferBudget), () => setFunds(null))
  }, [game.mode, game.info?.date])
  const [kind, setKind] = useState<PersonKind>('players')
  // Aba de olheiro aberta (1–7), só num jogo em curso
  const [scout, setScout] = useState<number | null>(null)
  const scouting = game.mode === 'game'
  // Aba Lista Preferencial: a pesquisa de jogadores só com os da lista
  const [shortlist, setShortlist] = useState(false)
  const pickTab = (k: string) => {
    if (k.startsWith('scout')) {
      setScout(Number(k.slice(5)))
      return
    }
    setScout(null)
    setShortlist(k === 'shortlist')
    setKind(k === 'shortlist' ? 'players' : (k as PersonKind))
    goPage(0)
  }
  const [filters, setFilters] = useState<Filters>(EMPTY_FILTERS)
  const [sort, setSort] = useState<SearchSort>('ability')
  const [status, setStatus] = useState<Status>('all')
  const [page, setPage] = useState(0)
  const [selected, setSelected] = useState(0)
  const [dialog, setDialog] = useState(false)

  const [nations, setNations] = useState<NationOption[]>([])
  const [found, setResult] = useState<SearchResult | null>(null)
  const [error, setError] = useState<string | null>(null)
  const requestId = useRef(0)
  const tableRef = useRef<HTMLDivElement>(null)
  const nameRef = useRef<HTMLInputElement>(null)

  const debouncedText = useDebounced(filters.text, 120)

  useEffect(() => {
    api.listNations().then(setNations, (e) => setError(String(e)))
  }, [])

  const goPage = (p: number) => {
    setPage(p)
    setSelected(0)
    tableRef.current?.scrollTo({ top: 0 })
  }
  const applyFilters = (f: Filters) => {
    setFilters(f)
    goPage(0)
  }

  const { nationId, age, positionGroup, side, expiring, injured, interested, ownSquad } = filters
  const own = game.info?.club.id
  // Sem nome, filtro, "Vêr" ou Lista Preferencial a tela fica vazia: só se procura com critérios
  const idle =
    !shortlist &&
    (status === 'all' || kind !== 'players') &&
    !debouncedText.trim() &&
    nationId === null &&
    age === null &&
    positionGroup === null &&
    side === null &&
    !expiring &&
    injured === null &&
    !interested &&
    ownSquad === EMPTY_FILTERS.ownSquad
  useEffect(() => {
    if (idle) {
      // Descarta a resposta de uma busca ainda a caminho
      requestId.current++
      return
    }
    const query = {
      ...toQuery({ text: debouncedText, nationId, age, positionGroup, side, expiring, injured, interested, ownSquad }, kind, own),
      status: shortlist ? ('shortlist' as const) : status === 'all' || kind !== 'players' ? undefined : status,
      sort,
      limit: PAGE_SIZE,
      offset: page * PAGE_SIZE,
    }
    const id = ++requestId.current
    api.searchPeople(query).then(
      (r) => {
        if (id !== requestId.current) return // resposta de uma busca antiga
        setResult(r)
        setError(null)
      },
      (e) => id === requestId.current && setError(String(e)),
    )
  }, [idle, debouncedText, nationId, age, positionGroup, side, expiring, injured, interested, ownSquad, own, kind, status, sort, page, shortlist])

  // Parada (sem critérios), a tela não mostra a última busca
  const result = idle ? null : found
  const total = result?.total ?? 0
  const pages = Math.max(1, Math.ceil(total / PAGE_SIZE))
  const count = result?.items.length ?? 0
  const players = kind === 'players'

  const open = (p: PersonSummary | undefined) => p && nav.go({ kind: 'person', id: p.id })

  // Teclado: setas movem a seleção, PageUp/PageDown trocam de página, digitar busca por nome, Esc limpa
  useEffect(() => {
    if (dialog || !active || scout !== null) return
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey) return // Alt+setas = navegação entre telas
      const inField = document.activeElement instanceof HTMLInputElement || document.activeElement instanceof HTMLSelectElement
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault()
        setSelected((s) => {
          const next = Math.max(0, Math.min(count - 1, s + (e.key === 'ArrowDown' ? 1 : -1)))
          tableRef.current?.querySelector(`[data-row="${next}"]`)?.scrollIntoView({ block: 'nearest' })
          return next
        })
      } else if (e.key === 'Enter' && !(document.activeElement instanceof HTMLSelectElement)) {
        open(result?.items[selected])
      } else if (e.key === 'PageDown' && page + 1 < pages) {
        e.preventDefault()
        goPage(page + 1)
      } else if (e.key === 'PageUp' && page > 0) {
        e.preventDefault()
        goPage(page - 1)
      } else if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !inField) {
        nameRef.current?.focus()
      } else if (e.key === 'Escape') {
        applyFilters({ ...filters, text: '' })
        nameRef.current?.focus()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  const nationName = (id: number) => nations.find((n) => n.id === id)?.name
  // Os filtros ativos, em etiquetas na barra roxa
  const chips = [status !== 'all' && players ? STATUSES.find((s) => s.key === status)!.label : null, ...describe(filters, nationName).split(' · ')]
    .filter((t): t is string => !!t && t !== 'Todos')
  const barButton =
    'flex h-8 items-center gap-1.5 rounded-[4px] border border-white/20 bg-black/25 px-3 text-[13px] font-semibold text-white hover:bg-black/40 disabled:text-white/30'

  return (
    <div className="relative flex h-full flex-col">
      <ScreenHeader
        icon={<Search size={22} />}
        title="Pesquisa Jogador & Staff"
        right={
          funds !== null && (
            <span className="rounded-md bg-black/40 px-3 py-1.5 text-[13px] text-[#c8c8d0]">
              Fundos: <b className="text-cm-yellow">{money(funds) || '€0'}</b>
            </span>
          )
        }
      />
      <Tabs
        tabs={TABS.map((t) => (t.key.startsWith('scout') || t.key === 'shortlist' ? { ...t, disabled: !scouting } : t))}
        active={scout !== null ? `scout${scout}` : shortlist ? 'shortlist' : kind}
        onChange={pickTab}
      />
      {scout !== null && <ScoutPanel key={scout} slot={scout} nations={nations} />}
      {scout === null && (
        <>

      <div className="mx-2.5 mt-2.5 flex h-11 shrink-0 items-center gap-2 rounded-md bg-cm-purple px-2">
        <button className={barButton} onClick={() => setDialog(true)}>
          Filtros <ChevronDown size={14} />
        </button>
        {kind === 'players' && (
          <PurpleMenu
            label="Vêr"
            options={STATUSES}
            value={status}
            onChange={(s) => {
              setStatus(s)
              goPage(0)
            }}
            buttonClass={barButton}
            trigger={
              <>
                Vêr <ChevronDown size={14} />
              </>
            }
          />
        )}
        <label className="ml-1 flex items-center gap-2 text-[13px] font-semibold text-white">
          Nome:
          <input
            ref={nameRef}
            autoFocus
            value={filters.text}
            onChange={(e) => applyFilters({ ...filters, text: e.target.value })}
            placeholder="Filtrar por nome…"
            className="h-8 w-56 rounded-[4px] border border-white/15 bg-[#0d0e13] px-2.5 text-cm-yellow outline-none placeholder:text-[#55565f] focus:border-[#4edea3]"
          />
        </label>
        <div className="flex min-w-0 flex-1 items-center gap-1.5 overflow-hidden">
          {chips.map((c) => (
            <span key={c} className="shrink-0 rounded-[4px] border border-white/15 bg-black/25 px-2 py-0.5 text-[12px] text-white">
              {c}
            </span>
          ))}
        </div>
        <div className="flex shrink-0 items-center gap-0.5 rounded-[4px] bg-black/40 px-1.5 py-1">
          <PageButton label="◀◀" onClick={() => goPage(0)} disabled={page === 0} />
          <PageButton label="◀" onClick={() => goPage(page - 1)} disabled={page === 0} />
          <span className="w-24 text-center font-bold tabular-nums text-cm-yellow">
            Pg {page + 1}/{pages}
          </span>
          <PageButton label="▶" onClick={() => goPage(page + 1)} disabled={page + 1 >= pages} />
          <PageButton label="▶▶" onClick={() => goPage(pages - 1)} disabled={page + 1 >= pages} />
        </div>
      </div>

      <div className="mx-2.5 mt-2.5 flex min-h-0 flex-1 flex-col overflow-hidden rounded-md bg-[#1a1b21]">
        <ErrorBox>{error}</ErrorBox>
        <div ref={tableRef} className={`min-h-0 flex-1 overflow-auto ${idle ? 'hidden' : ''}`}>
          <table className="w-full table-fixed border-collapse">
            <colgroup>
              <col />
              <col className="w-[22%]" />
              <col className="w-[15%]" />
              <col className="w-[13%]" />
              <col className="w-20" />
              <col className="w-14" />
              <col className="w-28" />
            </colgroup>
            <thead className="sticky top-0 z-10">
              <tr className="h-8">
                <Th label="Nome" sort="name" current={sort} onSort={setSort} className="text-left" />
                <Th label="Clube" current={sort} onSort={setSort} className="text-left" />
                <Th label="Nação" current={sort} onSort={setSort} className="text-left" />
                <Th label={players ? 'Posição' : 'Cargo'} current={sort} onSort={setSort} className="text-left" />
                <Th label="Inf" current={sort} onSort={setSort} />
                <Th label="Idade" sort="age" current={sort} onSort={setSort} />
                <Th label="Valor" sort="ability" current={sort} onSort={setSort} />
              </tr>
            </thead>
            <tbody>
              {result?.items.map((p, i) => (
                <Row
                  key={p.id}
                  p={p}
                  index={i}
                  selected={i === selected}
                  onSelect={() => setSelected(i)}
                  onOpen={() => open(p)}
                  onOpenClub={(club) => nav.go({ kind: 'club', id: club })}
                />
              ))}
            </tbody>
          </table>
        </div>
        {idle && (
          <div className="flex flex-1 flex-col items-center justify-center gap-2 text-center text-[#c8c8d0]">
            <Search size={28} className="text-[#55565f]" />
            <span className="text-[15px]">Escreva um nome ou abra os Filtros para pesquisar.</span>
          </div>
        )}
        <div className={`shrink-0 border-t border-cm-line py-1.5 text-center font-title text-xl font-bold text-cm-magenta ${idle ? 'hidden' : ''}`}>
          {total.toLocaleString('pt-BR')} {players ? 'jogadores' : 'pessoas'} encontrados
        </div>
      </div>

        </>
      )}
      {scouting && (
        <div className="mt-2.5">
          <Tabs tabs={MORE_SCOUTS} active={scout !== null ? `scout${scout}` : ''} onChange={pickTab} />
        </div>
      )}
      <BottomBar active={active} onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />

      {dialog && (
        <FilterDialog
          initial={filters}
          players={players}
          nations={nations}
          onCancel={() => setDialog(false)}
          onOk={(f) => {
            applyFilters(f)
            setDialog(false)
          }}
        />
      )}
    </div>
  )
}
