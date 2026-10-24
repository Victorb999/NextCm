import { useEffect, useMemo, useState } from 'react'
import { api } from '../api'
import type { NumberAssign } from '../bindings/NumberAssign'
import type { SquadNumbers } from '../bindings/SquadNumbers'
import { BottomBar, PurpleMenu, TitleBand } from '../components/Screen'
import { shortListName } from '../format'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'

// "Números Plantel", como no CM: escolher um jogador e clicar num número livre da grelha.
// Auto dá números a quem não tem (pelas preferências, no core); Anular volta ao gravado;
// Enviar grava.

type View = 'missing' | 'all'
type Criterion = 'position' | 'name' | 'number'

const VIEWS: { key: View; label: string }[] = [
  { key: 'missing', label: 'Sem número' },
  { key: 'all', label: 'Todos' },
]
const CRITERIA: { key: Criterion; label: string }[] = [
  { key: 'position', label: 'Posição' },
  { key: 'name', label: 'Nome' },
  { key: 'number', label: 'Número' },
]

/** Uma linha de 25 números: os livres claros, os ocupados apagados, o do escolhido amarelo. */
function NumberRow({
  from,
  taken,
  current,
  onPick,
}: {
  from: number
  taken: Set<number>
  current: number | null
  onPick: (n: number) => void
}) {
  return (
    <div className="flex gap-0.5 px-2">
      {Array.from({ length: 25 }, (_, i) => from + i)
        .filter((n) => n <= 99)
        .map((n) => {
          const free = !taken.has(n)
          return (
            <button
              key={n}
              disabled={!free && n !== current}
              onClick={() => onPick(n)}
              className={`h-6 w-7 border border-black text-[13px] tabular-nums ${
                n === current
                  ? 'bg-cm-yellow font-bold text-black'
                  : free
                    ? 'bg-[#2b2b2b] font-bold text-white hover:bg-cm-sel'
                    : 'bg-[#1a1a1a] text-[#555]'
              }`}
            >
              {n}
            </button>
          )
        })}
    </div>
  )
}

export default function SquadNumbersPage() {
  const nav = useNav()
  const game = useGame()
  useEscBack()
  const [saved, setSaved] = useState<SquadNumbers | null>(null)
  const [numbers, setNumbers] = useState<Map<number, number | null>>(new Map())
  const [selected, setSelected] = useState<number | null>(null)
  const [view, setView] = useState<View>('all')
  const [criterion, setCriterion] = useState<Criterion>('position')
  const [error, setError] = useState<string | null>(null)

  const reset = (s: SquadNumbers) => {
    setSaved(s)
    setNumbers(new Map(s.players.map((p) => [p.id, p.number])))
    // Começa no primeiro sem número
    setSelected(s.players.find((p) => p.number === null)?.id ?? null)
    if (s.players.some((p) => p.number === null)) setView('missing')
  }
  useEffect(() => {
    api.squadNumbers().then(reset, (e) => setError(String(e)))
  }, [])

  const taken = useMemo(() => new Set([...numbers.values()].filter((n): n is number => n !== null)), [numbers])
  const players = useMemo(() => {
    if (!saved) return []
    const list = saved.players.filter((p) => view === 'all' || numbers.get(p.id) === null || p.id === selected)
    const num = (id: number) => numbers.get(id) ?? 999
    if (criterion === 'name') list.sort((a, b) => a.listName.localeCompare(b.listName))
    if (criterion === 'number') list.sort((a, b) => num(a.id) - num(b.id))
    if (criterion === 'position') list.sort((a, b) => a.positionOrder - b.positionOrder || a.listName.localeCompare(b.listName))
    return list
  }, [saved, numbers, view, criterion, selected])

  const current = selected !== null ? (numbers.get(selected) ?? null) : null
  const pick = (n: number) => {
    if (selected === null) return
    const next = new Map(numbers)
    next.set(selected, n === current ? null : n)
    setNumbers(next)
    // Passa ao seguinte sem número
    const after = saved?.players.find((p) => p.id !== selected && next.get(p.id) === null)
    if (n !== current && after) setSelected(after.id)
  }
  const assigns = (): NumberAssign[] => [...numbers.entries()].map(([id, number]) => ({ id, number }))
  const auto = async () => {
    try {
      const got = await api.autoSquadNumbers(assigns())
      setNumbers(new Map(got.map((a) => [a.id, a.number])))
      setError(null)
    } catch (e) {
      setError(String(e))
    }
  }
  const send = async () => {
    try {
      reset(await api.setSquadNumbers(assigns()))
      const info = await api.gameInfo()
      if (info) game.update(info)
      nav.back()
    } catch (e) {
      setError(String(e))
    }
  }

  return (
    <div className="flex h-full flex-col">
      <TitleBand title={saved ? `Números Plantel ${saved.club.shortName}` : ''} fg="#000000" bg="#ffffff" />
      <div className="py-2 text-center font-title text-[30px] font-bold text-cm-yellow">Por Favor Escolha</div>
      <div className="flex shrink-0 items-center gap-1 px-1.5">
        <PurpleMenu label="Vêr" options={VIEWS} value={view} onChange={setView} />
        <PurpleMenu label="Critério" options={CRITERIA} value={criterion} onChange={setCriterion} />
        <div className="flex-1" />
        <button className="cm-purple h-6 w-40" onClick={() => void auto()}>
          Auto
        </button>
        <button className="cm-purple h-6 w-40" onClick={() => saved && reset(saved)}>
          Anular
        </button>
      </div>
      <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 flex-col border border-cm-line bg-cm-panel/80 py-1.5">
        <div className="pb-1.5 text-center font-title text-xl font-bold text-cm-yellow">{CRITERIA.find((c) => c.key === criterion)!.label}</div>
        <NumberRow from={1} taken={taken} current={current} onPick={pick} />
        <div className="mt-0.5">
          <NumberRow from={26} taken={taken} current={current} onPick={pick} />
        </div>
        {error && <div className="mx-2 mt-1 bg-red-900 px-2 py-1 text-white">{error}</div>}
        <div className="my-1.5 min-h-0 flex-1 overflow-auto px-2">
          {players.length === 0 && <div className="p-4 text-center text-cm-magenta">Todos os jogadores têm número.</div>}
          {players.map((p) => {
            const n = numbers.get(p.id) ?? null
            return (
              <div
                key={p.id}
                onClick={() => setSelected(p.id)}
                className={`flex h-[26px] cursor-pointer items-center gap-3 ${p.id === selected ? 'bg-cm-sel' : ''}`}
              >
                <span className="flex h-[22px] w-10 shrink-0 items-center justify-center cm-cell text-white">{n ?? ''}</span>
                <span className="w-72 truncate font-title text-[19px] text-white">{shortListName(p.listName)}</span>
                <span className="text-cm-yellow">{p.position}</span>
              </div>
            )
          })}
        </div>
        <NumberRow from={51} taken={taken} current={current} onPick={pick} />
        <div className="mt-0.5">
          <NumberRow from={76} taken={taken} current={current} onPick={pick} />
        </div>
      </div>
      <BottomBar right="Enviar" onLeft={nav.back} onRight={saved ? () => void send() : undefined} />
    </div>
  )
}
