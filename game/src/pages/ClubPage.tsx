import { useEffect, useMemo, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { ClubProfile } from '../bindings/ClubProfile'
import type { ClubTactics } from '../bindings/ClubTactics'
import type { Fixture } from '../bindings/Fixture'
import { ClubFixtureList, MatchInfo } from '../components/Fixtures'
import FinancesPanel from '../components/FinancesPanel'
import TransfersPanel from '../components/TransfersPanel'
import { BottomBar, Link, PurpleMenu, Tabs, TitleBand } from '../components/Screen'
import { money, shortListName } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'
import { BENCH, selectionOf, spotOf, swap, useDragSwap, type Spot } from '../selection'

const TABS = [
  { key: 'squad', label: 'Plantel' },
  { key: 'transfers', label: 'Transferências' },
  { key: 'next', label: 'Próximo Jogo' },
  { key: 'fixtures', label: 'Calendário' },
  { key: 'info', label: 'Finanças & Info' },
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

/** Uma linha do plantel: [número] [lugar/Int] Nome ... posição */
type Entry = { id: number; number: ReactNode; badge: ReactNode; name: string; detail: string; spot?: Spot }

type Bind = { 'data-spot': string; onMouseDown: (e: React.MouseEvent) => void }

/** "D D" -> "DD", "M C" -> "MC": a sigla curta da função, como nas casas do plantel do CM. */
const shortRole = (role: string) => role.replace(' ', '')

const byNullsLast = (a: number | null, b: number | null) => (a ?? 999) - (b ?? 999)

function SquadGrid({
  entries,
  selected,
  onSelect,
  onOpen,
  bind,
  onToggle,
}: {
  entries: Entry[]
  selected: number
  onSelect: (i: number) => void
  onOpen: (e: Entry) => void
  /** Só no clube do treinador: arrastar para mudar o lugar na equipa. */
  bind?: (spot: Spot, label: string) => Bind
  /** Só no clube do treinador: clicar no número ou na casa põe/tira da equipa. */
  onToggle?: (e: Entry) => void
}) {
  return (
    <div className="grid grid-cols-2 gap-x-3 px-2">
      {entries.map((e, i) => {
        const drag = bind && e.spot ? bind(e.spot, e.name) : undefined
        return (
        <div
          key={e.id}
          data-row={i}
          data-spot={drag?.['data-spot']}
          onMouseDown={drag?.onMouseDown}
          onClick={() => onSelect(i)}
          onDoubleClick={() => onOpen(e)}
          className={`flex h-[26px] items-center gap-1 ${i === selected ? 'bg-cm-sel' : ''} ${drag ? 'cursor-grab' : ''}`}
        >
          <span
            onMouseDown={onToggle && ((ev) => ev.stopPropagation())}
            onClick={onToggle && (() => onToggle(e))}
            className={`flex h-[22px] w-10 shrink-0 items-center justify-center cm-cell text-white ${onToggle ? 'cursor-pointer' : ''}`}
          >
            {e.number}
          </span>
          <span
            onMouseDown={onToggle && ((ev) => ev.stopPropagation())}
            onClick={onToggle && (() => onToggle(e))}
            title={onToggle ? 'Clique para pôr/tirar da equipa' : undefined}
            className={`flex h-[22px] w-9 shrink-0 items-center justify-center cm-cell text-xs text-cm-yellow ${onToggle ? 'cursor-pointer' : ''}`}
          >
            {e.badge}
          </span>
          <span className="ml-1 flex-1 truncate font-title text-[18px] text-white">{e.name}</span>
          <span className={`w-28 shrink-0 truncate ${i === selected ? 'text-white' : 'text-cm-yellow'}`}>{e.detail}</span>
        </div>
        )
      })}
    </div>
  )
}

/** Finanças pela palavra, como o CM (o banco guarda o saldo em euros). */
function financesLabel(cash: number | null) {
  if (cash === null) return '-'
  if (cash >= 50_000_000) return 'Rico'
  if (cash >= 10_000_000) return 'Seguro'
  if (cash >= 1_000_000) return 'Razoável'
  if (cash >= 0) return 'Apertado'
  return 'Em dívida'
}

/** "Finanças & Info" → Informação Geral do CM: dados do clube e a lista do staff. */
function Info({ c, finances }: { c: ClubProfile; finances?: ReactNode }) {
  const nav = useNav()
  const row = (label: string, value: ReactNode) => (
    <div key={label} className="flex min-h-[26px] items-baseline gap-4">
      <span className="w-60 shrink-0 font-title text-[21px] leading-tight text-white">{label}</span>
      <span className="flex-1 text-[15px] text-cm-yellow">{value}</span>
    </div>
  )
  const competition = c.division?.name
  return (
    <div className="min-h-0 flex-1 overflow-auto px-4 pb-3">
      {finances}
      <div className="py-1.5 text-center font-title text-xl font-bold text-cm-yellow">Informação Geral</div>
      {row('País', c.nationId !== null && c.nation ? <Link onClick={() => nav.go({ kind: 'nation', id: c.nationId! })}>{c.nation}</Link> : '-')}
      {row('Estatuto', `${c.status ?? '-'}${competition ? ` (${competition})` : ''}`)}
      {row('Finanças', `${financesLabel(c.cash)} · saldo ${money(c.cash)}`)}
      {row('Estádio', [c.stadium, c.stadiumCity].filter(Boolean).join(', ') || '-')}
      {row(
        'Lotação',
        c.stadiumCapacity
          ? `${c.stadiumCapacity.toLocaleString('pt-BR')}${c.stadiumSeated === c.stadiumCapacity ? ' - todos lugares sentados' : c.stadiumSeated ? ` (${c.stadiumSeated.toLocaleString('pt-BR')} sentados)` : ''}`
          : '-',
      )}
      {row('Reputação', c.reputation ?? '-')}
      {c.reserveDivision &&
        row('Estadual', <Link onClick={() => nav.go({ kind: 'competition', id: c.reserveDivision!.id })}>{c.reserveDivision.name}</Link>)}
      {row(
        'Rivais',
        c.rivals.length
          ? c.rivals.map((r, i) => (
              <span key={r.id}>
                {i > 0 && ', '}
                <Link onClick={() => nav.go({ kind: 'club', id: r.id })}>{r.shortName}</Link>
              </span>
            ))
          : '-',
      )}
      <div className="mb-1 mt-3 inline-block border-b-2 border-cm-orange font-title text-[20px] font-bold text-cm-orange">Staff</div>
      {c.staff.map((s) =>
        row(
          // "Sobrenome, Nome" -> "Nome Sobrenome", como na lista do CM
          s.listName.includes(', ') ? s.listName.split(', ').reverse().join(' ') : s.listName,
          <Link onClick={() => nav.go({ kind: 'person', id: s.id })}>{s.job ?? '-'}</Link>,
        ),
      )}
    </div>
  )
}

export default function ClubPage({ id }: { id: number }) {
  const nav = useNav()
  const game = useGame()
  const [club, setClub] = useState<ClubProfile | null>(null)
  const [fixtures, setFixtures] = useState<Fixture[]>([])
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState('squad')
  const [view, setView] = useState<View>('players')
  const [criterion, setCriterion] = useState<Criterion>('position')
  const [selected, setSelected] = useState(0)
  const [tactics, setTactics] = useState<ClubTactics | null>(null)
  const gridRef = useRef<HTMLDivElement>(null)
  const mine = game.mode === 'game' && game.info?.club.id === id

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
    const sel = tactics && selectionOf(tactics)
    return players.map((p) => {
      const spot = sel ? spotOf(sel, p.id) : undefined
      // Casa na equipa (GR, DD... e SB1–SB7 nos suplentes); senão Les/Sus/Int como no CM
      const place =
        spot?.kind === 'starter' ? shortRole(tactics!.starters[spot.index].role) : spot?.kind === 'bench' ? `SB${spot.index + 1}` : null
      return {
      id: p.id,
      spot,
      number: criterion === 'age' ? (p.age ?? '') : criterion === 'ability' ? (p.currentAbility ?? '') : (p.squadNumber ?? ''),
      // Num jogo em curso, os indisponíveis aparecem como no CM: "Les" / "Sus"
      badge: p.injured ? (
        <span className="bg-[#b8560c] px-0.5 font-bold text-white">Les</span>
      ) : p.suspended ? (
        <span className="bg-[#9c0000] px-0.5 font-bold text-white">Sus</span>
      ) : (
        (place ?? '')
      ),
      name: shortListName(p.listName),
      detail: p.position,
      }
    })
  }, [club, view, criterion, tactics])

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
      <Tabs
        tabs={TABS.map((t) =>
          t.key === 'next' || t.key === 'fixtures' ? { ...t, disabled: fixtures.length === 0 } : t.key === 'transfers' ? { ...t, disabled: game.mode !== 'game' } : t,
        )}
        active={tab}
        onChange={setTab}
      />

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
            {/* Casas da equipa: arraste um jogador para uma casa (ou para outro jogador) para mudar o lugar */}
            {mine && tactics && view === 'players' && (
              <div className="flex shrink-0 flex-wrap gap-0.5 px-2 pb-1.5">
                {tactics.starters.map((s, i) => {
                  const b = drag.bind({ kind: 'starter', index: i }, shortRole(s.role))
                  return (
                    <span key={`s${i}`} {...b} title={s.player?.listName ?? 'vazio'} className="cm-chip min-w-[38px] cursor-grab px-1 text-center text-[12px]">
                      {shortRole(s.role)}
                    </span>
                  )
                })}
                {/* Banco fixo de 9, como no CM: arraste um jogador para uma casa SB vazia para o pôr no banco */}
                {Array.from({ length: BENCH }, (_, i) => {
                  const p = tactics.bench[i]
                  const b = drag.bind({ kind: 'bench', index: i }, `SB${i + 1}`)
                  return (
                    <span
                      key={`b${i}`}
                      {...b}
                      title={p?.listName ?? 'vazio'}
                      className={`min-w-[38px] cursor-grab px-1 text-center text-[12px] ${p ? 'cm-chip' : 'cm-chip !bg-[#2a1a45] !text-[#8a63c4]'}`}
                    >
                      SB{i + 1}
                    </span>
                  )
                })}
                <span className="ml-2 self-center text-[11px] text-[#aab3c4]">Arraste para trocar · clique no número ou na casa para pôr/tirar</span>
              </div>
            )}
            <div ref={gridRef} className="min-h-0 flex-1 overflow-auto pb-2">
              {entries.length ? (
                <SquadGrid
                  entries={entries}
                  selected={selected}
                  onSelect={setSelected}
                  onOpen={open}
                  bind={mine && tactics ? drag.bind : undefined}
                  onToggle={mine && tactics && view === 'players' ? (e) => void toggle(e) : undefined}
                />
              ) : (
                <div className="p-6 text-center text-cm-magenta">Ninguém registado.</div>
              )}
            </div>
          </>
        )}
        {club && tab === 'info' && <Info c={club} finances={mine ? <FinancesPanel clubId={id} /> : undefined} />}
        {tab === 'next' && (
          <MatchInfo key="next" clubId={id} fixtures={fixtures} start={nextMatch ? fixtures.indexOf(nextMatch) : fixtures.length} empty="Não há mais jogos marcados." />
        )}
        {tab === 'last' && (
          <MatchInfo key="last" clubId={id} fixtures={fixtures} start={lastMatch ? fixtures.indexOf(lastMatch) : -1} empty="Ainda não jogou." />
        )}
        {tab === 'transfers' && <TransfersPanel clubId={id} />}
        {tab === 'fixtures' && <ClubFixtureList clubId={id} fixtures={fixtures} />}
      </div>

      {/* Atalhos inferiores do CM; o Treino chega no marco 3.3 */}
      <div className="mt-1.5">
        <Tabs
          tabs={[
            { key: 'tactics', label: 'Tácticas' },
            { key: 'training', label: 'Treino', disabled: true },
            { key: 'last', label: 'Último Jogo', disabled: !lastMatch },
            { key: 'division', label: division?.shortName ?? '-', disabled: !division },
            { key: 'history', label: 'Historial' },
          ]}
          active={tab === 'last' ? 'last' : ''}
          onChange={(k) => {
            if (k === 'last') setTab('last')
            if (k === 'division' && division) nav.go({ kind: 'competition', id: division.id })
            if (k === 'tactics') nav.go({ kind: 'tactics', id })
            if (k === 'history') nav.go({ kind: 'clubHistory', id })
          }}
        />
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
      {drag.ghost}
    </div>
  )
}
