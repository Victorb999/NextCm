import { ChevronDown } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { api } from '../api'
import type { NationOption } from '../bindings/NationOption'
import type { PersonKind } from '../bindings/PersonKind'
import type { PersonSummary } from '../bindings/PersonSummary'
import type { SearchResult } from '../bindings/SearchResult'
import type { SearchSort } from '../bindings/SearchSort'
import { BottomBar, Tabs, TitleBand } from '../components/Screen'
import FilterDialog from './FilterDialog'
import { EMPTY_FILTERS, describe, toQuery, type Filters } from './searchFilters'

const PAGE_SIZE = 100

const TABS = [
  { key: 'shortlist', label: 'Lista Preferencial', disabled: true },
  { key: 'players', label: 'Pesquisar Jogador' },
  { key: 'staff', label: 'Pesquisa Staff' },
  { key: 'scout1', label: 'Olheiro #1', disabled: true },
  { key: 'scout2', label: 'Olheiro #2', disabled: true },
]

function useDebounced<T>(value: T, ms: number) {
  const [debounced, setDebounced] = useState(value)
  useEffect(() => {
    const t = setTimeout(() => setDebounced(value), ms)
    return () => clearTimeout(t)
  }, [value, ms])
  return debounced
}

// PA negativo no CM = faixa aleatória definida pelo jogo
const ability = (v: number | null) => (v === null || v <= 0 ? '-' : v)

function Row({ p, index, selected, onSelect }: { p: PersonSummary; index: number; selected: boolean; onSelect: () => void }) {
  return (
    <tr
      className={`h-[22px] border-b border-cm-line ${selected ? 'bg-cm-sel' : ''}`}
      onMouseDown={onSelect}
      data-row={index}
    >
      <td className="truncate px-2 text-white">{p.listName}</td>
      <td className={`truncate px-2 ${selected ? 'text-white' : 'text-cm-magenta'}`}>{p.club ?? 'Sem clube'}</td>
      <td className={`truncate px-2 ${selected ? 'text-white' : 'text-cm-yellow'}`}>{p.nation ?? '-'}</td>
      <td className={`truncate px-2 ${selected ? 'text-white' : 'text-cm-orange'}`}>{p.position || p.job}</td>
      <td className="px-2 text-center tabular-nums">{p.age ?? '-'}</td>
      <td className="bg-black/40 px-2 text-center tabular-nums">{ability(p.currentAbility)}</td>
      <td className="bg-black/40 px-2 text-center tabular-nums">{ability(p.potentialAbility)}</td>
    </tr>
  )
}

/** Cabeçalho roxo de coluna; clicar ordena pela coluna (quando ela permite). */
function Th({ label, sort, current, onSort, className = '' }: { label: string; sort?: SearchSort; current: SearchSort; onSort: (s: SearchSort) => void; className?: string }) {
  const active = sort !== undefined && sort === current
  return (
    <th
      onClick={sort ? () => onSort(sort) : undefined}
      className={`border-x border-black bg-cm-head px-2 font-normal ${sort ? 'cursor-pointer' : ''} ${active ? 'text-cm-yellow' : 'text-white'} ${className}`}
    >
      {label}
      {active && ' ▾'}
    </th>
  )
}

function PageButton({ label, onClick, disabled }: { label: string; onClick: () => void; disabled: boolean }) {
  return (
    <button onClick={onClick} disabled={disabled} className="px-1 font-bold text-cm-yellow disabled:text-[#555]">
      {label}
    </button>
  )
}

export default function SearchPage() {
  const [kind, setKind] = useState<PersonKind>('players')
  const [filters, setFilters] = useState<Filters>(EMPTY_FILTERS)
  const [sort, setSort] = useState<SearchSort>('ability')
  const [page, setPage] = useState(0)
  const [selected, setSelected] = useState(0)
  const [dialog, setDialog] = useState(false)

  const [nations, setNations] = useState<NationOption[]>([])
  const [result, setResult] = useState<SearchResult | null>(null)
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

  const { nationId, age, positionGroup, side } = filters
  useEffect(() => {
    const query = {
      ...toQuery({ text: debouncedText, nationId, age, positionGroup, side }, kind),
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
  }, [debouncedText, nationId, age, positionGroup, side, kind, sort, page])

  const total = result?.total ?? 0
  const pages = Math.max(1, Math.ceil(total / PAGE_SIZE))
  const count = result?.items.length ?? 0
  const players = kind === 'players'

  // Teclado: setas movem a seleção, PageUp/PageDown trocam de página, digitar busca por nome, Esc limpa
  useEffect(() => {
    if (dialog) return
    const onKey = (e: KeyboardEvent) => {
      const inField = document.activeElement instanceof HTMLInputElement || document.activeElement instanceof HTMLSelectElement
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault()
        setSelected((s) => {
          const next = Math.max(0, Math.min(count - 1, s + (e.key === 'ArrowDown' ? 1 : -1)))
          tableRef.current?.querySelector(`[data-row="${next}"]`)?.scrollIntoView({ block: 'nearest' })
          return next
        })
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

  return (
    <div className="relative flex h-full flex-col">
      <TitleBand title="Pesquisa Jogador & Staff" fg="#0d5a22" bg="#f5f500" />
      <Tabs
        tabs={TABS}
        active={kind}
        onChange={(k) => {
          setKind(k as PersonKind)
          goPage(0)
        }}
      />

      <div className="flex shrink-0 items-center gap-1 px-1.5 pt-2">
        <button className="cm-purple flex h-6 w-40 items-center justify-center gap-1" onClick={() => setDialog(true)}>
          Filtros <ChevronDown size={14} />
        </button>
        <label className="ml-2 flex items-center gap-1.5 text-white">
          Nome
          <input
            ref={nameRef}
            autoFocus
            value={filters.text}
            onChange={(e) => applyFilters({ ...filters, text: e.target.value })}
            className="cm-field h-6 w-52"
          />
        </label>
        <div className="flex-1" />
        <div className="flex items-center gap-0.5 bg-black/60 px-2 py-0.5">
          <PageButton label="◀◀" onClick={() => goPage(0)} disabled={page === 0} />
          <PageButton label="◀" onClick={() => goPage(page - 1)} disabled={page === 0} />
          <span className="w-24 text-center font-bold tabular-nums text-cm-yellow">
            Pg {page + 1}/{pages}
          </span>
          <PageButton label="▶" onClick={() => goPage(page + 1)} disabled={page + 1 >= pages} />
          <PageButton label="▶▶" onClick={() => goPage(pages - 1)} disabled={page + 1 >= pages} />
        </div>
      </div>

      <div className="mx-1.5 mt-1 flex min-h-0 flex-1 flex-col border border-cm-line bg-cm-panel/85">
        <div className="shrink-0 px-2 pt-1 text-center text-cm-magenta">{describe(filters, nationName)}</div>
        <div className="shrink-0 py-1 text-center font-title text-xl font-bold text-cm-yellow">
          {players ? 'Pesquisar Jogador' : 'Pesquisar Staff'}
        </div>
        {error && <div className="bg-red-900 px-2 py-1 text-white">{error}</div>}
        <div ref={tableRef} className="min-h-0 flex-1 overflow-auto">
          <table className="w-full table-fixed border-collapse">
            <colgroup>
              <col />
              <col className="w-[22%]" />
              <col className="w-[15%]" />
              <col className="w-[13%]" />
              <col className="w-14" />
              <col className="w-12" />
              <col className="w-12" />
            </colgroup>
            <thead className="sticky top-0 z-10">
              <tr className="h-6">
                <Th label="Nome" sort="name" current={sort} onSort={setSort} className="text-left" />
                <Th label="Clube" current={sort} onSort={setSort} className="text-left" />
                <Th label="Nação" current={sort} onSort={setSort} className="text-left" />
                <Th label={players ? 'Posição' : 'Cargo'} current={sort} onSort={setSort} className="text-left" />
                <Th label="Idade" sort="age" current={sort} onSort={setSort} />
                <Th label="CA" sort="ability" current={sort} onSort={setSort} />
                <Th label="PA" current={sort} onSort={setSort} />
              </tr>
            </thead>
            <tbody>
              {result?.items.map((p, i) => (
                <Row key={p.id} p={p} index={i} selected={i === selected} onSelect={() => setSelected(i)} />
              ))}
            </tbody>
          </table>
        </div>
        <div className="shrink-0 py-1.5 text-center font-title text-2xl font-bold text-cm-magenta">
          {total.toLocaleString('pt-BR')} {players ? 'jogadores' : 'pessoas'} encontrados
        </div>
      </div>

      <BottomBar />

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
