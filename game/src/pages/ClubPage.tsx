import { useEffect, useMemo, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { ClubProfile } from '../bindings/ClubProfile'
import { BottomBar, Link, PurpleMenu, Tabs, TitleBand } from '../components/Screen'
import { money, shortListName } from '../format'
import { useNav } from '../nav'

const TABS = [
  { key: 'squad', label: 'Plantel' },
  { key: 'transfers', label: 'Transferências', disabled: true },
  { key: 'next', label: 'Próximo Jogo', disabled: true },
  { key: 'fixtures', label: 'Calendário', disabled: true },
  { key: 'info', label: 'Informação Geral' },
]

type View = 'players' | 'staff'
type Criterion = 'position' | 'name' | 'number' | 'age' | 'ability'

const VIEWS: { key: View; label: string }[] = [
  { key: 'players', label: 'Jogadores' },
  { key: 'staff', label: 'Staff' },
]

const CRITERIA: { key: Criterion; label: string }[] = [
  { key: 'position', label: 'Posição' },
  { key: 'name', label: 'Nome' },
  { key: 'number', label: 'Número' },
  { key: 'age', label: 'Idade' },
  { key: 'ability', label: 'Habilidade' },
]

/** Uma linha do plantel: [número] [Int] Nome ... posição */
type Entry = { id: number; number: ReactNode; badge: ReactNode; name: string; detail: string }

const byNullsLast = (a: number | null, b: number | null) => (a ?? 999) - (b ?? 999)

function SquadGrid({
  entries,
  selected,
  onSelect,
  onOpen,
}: {
  entries: Entry[]
  selected: number
  onSelect: (i: number) => void
  onOpen: (e: Entry) => void
}) {
  return (
    <div className="grid grid-cols-2 gap-x-3 px-2">
      {entries.map((e, i) => (
        <div
          key={e.id}
          data-row={i}
          onMouseDown={() => onSelect(i)}
          onDoubleClick={() => onOpen(e)}
          className={`flex h-[26px] items-center gap-1 ${i === selected ? 'bg-cm-sel' : ''}`}
        >
          <span className="flex h-[22px] w-10 shrink-0 items-center justify-center bg-[#262626] text-white">{e.number}</span>
          <span className="flex h-[22px] w-8 shrink-0 items-center justify-center bg-[#262626] text-xs text-white">{e.badge}</span>
          <span className="ml-1 flex-1 truncate font-title text-[18px] text-white">{e.name}</span>
          <span className={`w-28 shrink-0 truncate ${i === selected ? 'text-white' : 'text-cm-yellow'}`}>{e.detail}</span>
        </div>
      ))}
    </div>
  )
}

function Info({ c }: { c: ClubProfile }) {
  const nav = useNav()
  const rows: [string, ReactNode][] = [
    ['Nome', c.club.name],
    ['Nação', c.nation ?? '-'],
    ['Divisão', c.division?.name ?? '-'],
    ...(c.reserveDivision ? ([['Estadual', c.reserveDivision.name]] as [string, ReactNode][]) : []),
    ['Estádio', c.stadium ? `${c.stadium}${c.stadiumCapacity ? ` (${c.stadiumCapacity.toLocaleString('pt-BR')})` : ''}` : '-'],
    ['Reputação', c.reputation ?? '-'],
    ['Saldo', money(c.cash)],
    [
      'Rivais',
      c.rivals.length
        ? c.rivals.map((r, i) => (
            <span key={r.id}>
              {i > 0 && ', '}
              <Link onClick={() => nav.go({ kind: 'club', id: r.id })} className="text-cm-magenta">
                {r.shortName}
              </Link>
            </span>
          ))
        : '-',
    ],
    ['Jogadores / Staff', `${c.players.length} / ${c.staff.length}`],
  ]
  return (
    <div className="mx-auto mt-4 w-full max-w-2xl px-1.5">
      {rows.map(([k, v]) => (
        <div key={k} className="flex h-8 items-center justify-between border-b border-cm-line bg-black/50 px-3 text-[15px]">
          <span className="text-white">{k}</span>
          <span className="font-bold text-cm-yellow">{v}</span>
        </div>
      ))}
    </div>
  )
}

export default function ClubPage({ id }: { id: number }) {
  const nav = useNav()
  const [club, setClub] = useState<ClubProfile | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState('squad')
  const [view, setView] = useState<View>('players')
  const [criterion, setCriterion] = useState<Criterion>('position')
  const [selected, setSelected] = useState(0)
  const gridRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    api.getClub(id).then(setClub, (e) => setError(String(e)))
  }, [id])

  const entries: Entry[] = useMemo(() => {
    if (!club) return []
    if (view === 'staff') {
      return club.staff.map((s) => ({ id: s.id, number: '', badge: '', name: shortListName(s.listName), detail: s.job ?? '' }))
    }
    const players = [...club.players]
    const name = (a: { listName: string }, b: { listName: string }) => a.listName.localeCompare(b.listName)
    if (criterion === 'position') players.sort((a, b) => a.positionOrder - b.positionOrder || name(a, b))
    if (criterion === 'name') players.sort(name)
    if (criterion === 'number') players.sort((a, b) => byNullsLast(a.squadNumber, b.squadNumber) || name(a, b))
    if (criterion === 'age') players.sort((a, b) => byNullsLast(a.age, b.age) || name(a, b))
    if (criterion === 'ability') players.sort((a, b) => (b.currentAbility ?? 0) - (a.currentAbility ?? 0) || name(a, b))
    return players.map((p) => ({
      id: p.id,
      number: criterion === 'age' ? (p.age ?? '') : criterion === 'ability' ? (p.currentAbility ?? '') : (p.squadNumber ?? ''),
      badge: p.international ? 'Int' : '',
      name: shortListName(p.listName),
      detail: p.position,
    }))
  }, [club, view, criterion])

  const open = (e: Entry | undefined) => e && nav.go({ kind: 'person', id: e.id })

  // Teclado: setas andam na grade de 2 colunas, Enter abre, Esc/Backspace voltam
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey) return
      const step = { ArrowUp: -2, ArrowDown: 2, ArrowLeft: -1, ArrowRight: 1 }[e.key]
      if (step !== undefined && tab === 'squad') {
        e.preventDefault()
        setSelected((s) => {
          const next = Math.max(0, Math.min(entries.length - 1, s + step))
          gridRef.current?.querySelector(`[data-row="${next}"]`)?.scrollIntoView({ block: 'nearest' })
          return next
        })
      } else if (e.key === 'Enter' && tab === 'squad') {
        open(entries[selected])
      } else if (e.key === 'Escape' || e.key === 'Backspace') {
        nav.back()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  const colours = club?.club.fg && club?.club.bg ? { fg: club.club.fg, bg: club.club.bg } : { fg: '#ffffff', bg: '#3a3a3a' }
  const division = club?.division

  return (
    <div className="flex h-full flex-col">
      <TitleBand title={club?.club.name ?? ''} fg={colours.fg} bg={colours.bg} />
      <Tabs tabs={TABS} active={tab} onChange={setTab} />

      {tab === 'squad' && (
        <div className="flex shrink-0 items-center gap-1 px-1.5 pt-2">
          <PurpleMenu
            label="Vêr"
            options={VIEWS}
            value={view}
            onChange={(v) => {
              setView(v)
              setSelected(0)
            }}
          />
          <PurpleMenu
            label="Critério"
            options={CRITERIA}
            value={criterion}
            disabled={view === 'staff'}
            onChange={(c) => {
              setCriterion(c)
              setSelected(0)
            }}
          />
          <div className="flex-1" />
          <PurpleMenu label="Filtrar" options={[]} disabled />
        </div>
      )}

      <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 flex-col border border-cm-line bg-cm-panel/80">
        {error && <div className="bg-red-900 px-2 py-1 text-white">{error}</div>}
        {club && tab === 'squad' && (
          <>
            <div className="shrink-0 py-1.5 text-center font-title text-xl font-bold text-cm-yellow">
              {view === 'staff' ? 'Staff' : CRITERIA.find((c) => c.key === criterion)!.label}
            </div>
            <div ref={gridRef} className="min-h-0 flex-1 overflow-auto pb-2">
              {entries.length ? (
                <SquadGrid entries={entries} selected={selected} onSelect={setSelected} onOpen={open} />
              ) : (
                <div className="p-6 text-center text-cm-magenta">Ninguém registado.</div>
              )}
            </div>
          </>
        )}
        {club && tab === 'info' && <Info c={club} />}
      </div>

      {/* Atalhos inferiores do CM; ficam ativos quando as telas existirem */}
      <div className="mt-1.5">
        <Tabs
          tabs={[
            { key: 'tactics', label: 'Tácticas', disabled: true },
            { key: 'training', label: 'Treino', disabled: true },
            { key: 'last', label: 'Último Jogo', disabled: true },
            { key: 'division', label: division?.shortName ?? '-', disabled: true },
            { key: 'history', label: 'Historial', disabled: true },
          ]}
          active=""
          onChange={() => {}}
        />
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
