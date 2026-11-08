import { Columns2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import { PLAYER_ATTRIBUTES } from '../attributes'
import type { PersonProfile } from '../bindings/PersonProfile'
import { BottomBar, Link, Panel, ScreenHeader, Tabs } from '../components/Screen'
import { cmDate, money, wage } from '../format'
import { useEscBack, useNav } from '../nav'

// Comparar Jogadores (3.19.1, item 21): a organização do FM08 (dados dos dois no topo e, por baixo, os
// atributos em Técnicos / Mentais / Físicos com barras, ou as estatísticas), no visual do CM 01/02.

const LABEL = Object.fromEntries(PLAYER_ATTRIBUTES)
const GROUPS: [string, string[]][] = [
  ['Técnicos', ['heading', 'crossing', 'tackling', 'finishing', 'dribbling', 'free_kicks', 'handling', 'marking', 'passing', 'long_shots', 'technique', 'reflexes']],
  ['Mentais', ['aggression', 'anticipation', 'bravery', 'vision', 'decisions', 'determination', 'leadership', 'flair', 'positioning', 'off_the_ball', 'teamwork', 'work_rate']],
  ['Físicos', ['acceleration', 'agility', 'balance', 'strength', 'jumping', 'stamina', 'pace']],
]

const GREEN = 'font-bold text-[#4cd964]'
const NEUTRAL = 'text-cm-orange'
const UNKNOWN = 'text-[#777]'

/** Cor do valor: o melhor dos dois a verde, o que o clube não conhece a cinza. */
function tone(v: number | null | undefined, other: number | null | undefined, lowerIsBetter = false): string {
  if (v == null) return UNKNOWN
  if (other == null || v === other) return NEUTRAL
  return (lowerIsBetter ? v < other : v > other) ? GREEN : NEUTRAL
}

/** Barra de 1 a 20; a do melhor com mais brilho. */
function Bar({ v, best }: { v: number | null | undefined; best: boolean }) {
  return (
    <div className="h-2.5 w-full rounded-sm bg-black/50">
      {v != null && <div className={`h-full rounded-sm ${best ? 'bg-[#4cd964]' : 'bg-[#8a5cd0]'}`} style={{ width: `${Math.min(100, (v / 20) * 100)}%` }} />}
    </div>
  )
}

const INFO = 'grid h-6 grid-cols-[1fr_10rem_1fr] items-center gap-x-2 px-4 text-[14px] tabular-nums'

function InfoRow({ label, a, b }: { label: string; a: [string, string?]; b: [string, string?] }) {
  return (
    <div className={INFO}>
      <span className={`truncate text-right ${a[1] ?? NEUTRAL}`}>{a[0]}</span>
      <span className="text-center text-white">{label}</span>
      <span className={`truncate ${b[1] ?? NEUTRAL}`}>{b[0]}</span>
    </div>
  )
}

function official(p: PersonProfile) {
  return p.season.find((s) => s.competition === 'Jogos Oficiais')
}

/** "Comparar Jogadores": dois jogadores lado a lado (o que o clube conhece deles). */
export default function ComparePage({ a, b }: { a: number; b: number }) {
  const nav = useNav()
  useEscBack()
  const [people, setPeople] = useState<[PersonProfile, PersonProfile] | null>(null)
  const [tab, setTab] = useState('attributes')
  useEffect(() => {
    Promise.all([api.getPerson(a), api.getPerson(b)]).then(([x, y]) => setPeople([x, y]), () => {})
  }, [a, b])

  const pair = <T,>(f: (p: PersonProfile) => T): [T, T] | null => (people ? [f(people[0]), f(people[1])] : null)
  const value = pair((p) => p.player?.value ?? null)
  const intl = pair((p) => p.internationalApps)
  const stats = pair((p) => official(p))

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader icon={<Columns2 size={22} />} title={people ? `${people[0].name} ~ ${people[1].name}` : 'Comparar Jogadores'} />
      {people && value && intl && stats && (
        <Panel className="mx-2 mb-1 min-h-0 flex-1">
          <div className="min-h-0 flex-1 overflow-auto">
            <div className="grid grid-cols-[1fr_10rem_1fr] items-center gap-x-2 border-b border-cm-line px-4 py-1 font-title text-[17px] font-bold">
              {people.map((p, i) => (
                <Link key={p.id} className={`truncate text-cm-yellow ${i === 0 ? 'text-right' : ''}`} onClick={() => nav.go({ kind: 'person', id: p.id })}>
                  {p.name}
                </Link>
              ))}
            </div>
            <div className="cm-zebra">
              <InfoRow label="Nacionalidade" a={[people[0].nationality ?? '-']} b={[people[1].nationality ?? '-']} />
              <InfoRow
                label="Internacional"
                a={[`${people[0].internationalApps} jogos / ${people[0].internationalGoals} golos`, tone(intl[0], intl[1])]}
                b={[`${people[1].internationalApps} jogos / ${people[1].internationalGoals} golos`, tone(intl[1], intl[0])]}
              />
              <InfoRow
                label="Nascimento"
                a={[`${cmDate(people[0].birthDate)} (${people[0].age ?? '-'} anos)`, tone(people[0].age, people[1].age, true)]}
                b={[`${cmDate(people[1].birthDate)} (${people[1].age ?? '-'} anos)`, tone(people[1].age, people[0].age, true)]}
              />
              <InfoRow label="Pé Preferido" a={[people[0].player?.preferredFoot ?? '-']} b={[people[1].player?.preferredFoot ?? '-']} />
              <InfoRow
                label="Clube"
                a={[people[0].club?.shortName ?? 'Sem clube', 'text-cm-magenta']}
                b={[people[1].club?.shortName ?? 'Sem clube', 'text-cm-magenta']}
              />
              <InfoRow label="Posição" a={[people[0].player?.positionLong || people[0].player?.position || '-']} b={[people[1].player?.positionLong || people[1].player?.position || '-']} />
              <InfoRow label="Valor" a={[value[0] != null ? money(value[0]) : '-', tone(value[0], value[1])]} b={[value[1] != null ? money(value[1]) : '-', tone(value[1], value[0])]} />
              <InfoRow label="Salário" a={[wage(people[0].wage)]} b={[wage(people[1].wage)]} />
              <InfoRow label="Contrato Termina" a={[cmDate(people[0].contractExpires)]} b={[cmDate(people[1].contractExpires)]} />
            </div>
            <div className="mt-1 px-2">
              <Tabs
                tabs={[
                  { key: 'attributes', label: 'Atributos' },
                  { key: 'stats', label: 'Estatísticas' },
                ]}
                active={tab}
                onChange={setTab}
              />
            </div>
            {tab === 'attributes' ? (
              <div className="grid grid-cols-3 gap-3 p-2">
                {GROUPS.map(([title, keys]) => (
                  <div key={title}>
                    <div className="mb-0.5 bg-[#3d1a73] px-2 text-center font-title text-[15px] leading-6 text-cm-yellow">{title}</div>
                    <div className="cm-zebra">
                      {keys.map((key) => {
                        const x = people[0].player?.attributes[key]
                        const y = people[1].player?.attributes[key]
                        return (
                          <div key={key} className="grid h-6 grid-cols-[1fr_1.6rem_3.2rem_3.2rem_1.6rem] items-center gap-x-1.5 px-2 text-[14px] tabular-nums">
                            <span className="truncate text-white">{LABEL[key] ?? key}</span>
                            <span className={`text-right ${tone(x, y)}`}>{x ?? '-'}</span>
                            <Bar v={x} best={x != null && (y == null || x >= y)} />
                            <Bar v={y} best={y != null && (x == null || y >= x)} />
                            <span className={`text-left ${tone(y, x)}`}>{y ?? '-'}</span>
                          </div>
                        )
                      })}
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="cm-zebra p-2">
                {(
                  [
                    ['Jogos', (s) => s?.apps ?? 0, false],
                    ['Golos', (s) => s?.goals ?? 0, false],
                    ['Assistências', (s) => s?.assists ?? 0, false],
                    ['Nota média', (s) => s?.rating ?? 0, false],
                  ] as [string, (s: ReturnType<typeof official>) => number, boolean][]
                ).map(([label, f]) => {
                  const x = f(stats[0])
                  const y = f(stats[1])
                  const show = (v: number) => (label === 'Nota média' ? (v ? v.toFixed(2) : '-') : String(v))
                  return <InfoRow key={label} label={label} a={[show(x), tone(x, y)]} b={[show(y), tone(y, x)]} />
                })}
                <div className="px-4 pt-2 text-center text-[12px] text-[#8a8a8a]">Jogos oficiais da época no clube.</div>
              </div>
            )}
            <div className="px-4 pb-1 text-center text-[12px] text-[#8a8a8a]">
              O melhor dos dois a verde; <span className={UNKNOWN}>-</span> é o que o clube ainda não conhece (precisa de mais observações).
            </div>
          </div>
        </Panel>
      )}
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
