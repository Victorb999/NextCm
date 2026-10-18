import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { PersonProfile } from '../bindings/PersonProfile'
import { BottomBar, Link, Tabs, TitleBand } from '../components/Screen'
import { cmDate, money } from '../format'
import { useNav } from '../nav'

// Atributos na ordem e com os nomes do perfil do CM 01/02 em português.
const PLAYER_ATTRIBUTES: [string, string][] = [
  ['acceleration', 'Aceleração'],
  ['agility', 'Agilidade'],
  ['aggression', 'Agressividade'],
  ['anticipation', 'Antecipação'],
  ['balance', 'Balanço'],
  ['free_kicks', 'Bolas Paradas'],
  ['bravery', 'Bravura'],
  ['heading', 'Cabeceamento'],
  ['vision', 'Criatividade'],
  ['crossing', 'Cruzamentos'],
  ['decisions', 'Decisões'],
  ['tackling', 'Desarme'],
  ['determination', 'Determinação'],
  ['finishing', 'Finalização'],
  ['dribbling', 'Finta'],
  ['strength', 'Força'],
  ['jumping', 'Impulsão'],
  ['work_rate', 'Índice Trabalho'],
  ['leadership', 'Influência'],
  ['flair', 'Instinto'],
  ['handling', 'Jogo Mãos'],
  ['marking', 'Marcação'],
  ['passing', 'Passe'],
  ['positioning', 'Posicionamento'],
  ['reflexes', 'Reflexos'],
  ['long_shots', 'Remates Longe'],
  ['stamina', 'Resistência'],
  ['off_the_ball', 'Sem Bola'],
  ['technique', 'Técnica'],
  ['teamwork', 'Trabalho Equipa'],
  ['pace', 'Velocidade'],
]

const STAFF_ATTRIBUTES: [string, string][] = [
  ['adaptability', 'Adaptabilidade'],
  ['ambition', 'Ambição'],
  ['attacking', 'Ataque'],
  ['judging_potential', 'Avaliar Potencial'],
  ['judgement', 'Avaliar Jogadores'],
  ['determination', 'Determinação'],
  ['discipline', 'Disciplina'],
  ['physiotherapy', 'Fisioterapia'],
  ['offside', 'Fora de Jogo'],
  ['free_roles', 'Funções Livres'],
  ['man_handling', 'Gestão Humana'],
  ['directness', 'Jogo Directo'],
  ['youngsters', 'Jovens'],
  ['loyalty', 'Lealdade'],
  ['marking', 'Marcação'],
  ['motivating', 'Motivação'],
  ['patience', 'Paciência'],
  ['pressing', 'Pressing'],
  ['professionalism', 'Profissionalismo'],
  ['resources', 'Recursos'],
  ['tactics', 'Táctica'],
  ['coaching', 'Treinar'],
  ['coaching_gks', 'Treinar GR'],
  ['coaching_technique', 'Treinar Técnica'],
]

const TABS = [
  { key: 'profile', label: 'Perfil' },
  { key: 'injuries', label: 'Lesões & Castigos', disabled: true },
  { key: 'contract', label: 'Contrato' },
  { key: 'transfer', label: 'Transferência', disabled: true },
  { key: 'history', label: 'Historial' },
]

const DEFAULT_COLOURS = { fg: '#ffffff', bg: '#3a3a3a' }

function valueColor(v: number) {
  return v >= 16 ? 'text-[#ff4433]' : 'text-cm-orange'
}

/** Grade de 3 colunas "Nome .... valor", como no perfil do CM. */
function AttributeGrid({ items }: { items: { label: string; value: ReactNode; color?: string }[] }) {
  const perColumn = Math.ceil(items.length / 3)
  const columns = [0, 1, 2].map((c) => items.slice(c * perColumn, (c + 1) * perColumn))
  return (
    <div className="grid grid-cols-3 gap-x-1 px-1.5">
      {columns.map((col, c) => (
        <div key={c}>
          {col.map((it) => (
            <div key={it.label} className="flex h-[23px] items-center justify-between border-b border-cm-line bg-black/50 px-2">
              <span className="text-[15px] text-white">{it.label}</span>
              <span className={`text-[15px] font-bold tabular-nums ${it.color ?? 'text-cm-orange'}`}>{it.value}</span>
            </div>
          ))}
        </div>
      ))}
    </div>
  )
}

/** Tabela pequena de cabeçalho roxo (resumo e histórico). */
function PurpleTable({ head, rows }: { head: string[]; rows: ReactNode[][] }) {
  return (
    <table className="w-full border-collapse text-center">
      <thead>
        <tr className="h-6">
          {head.map((h, i) => (
            <th key={i} className={`border border-black bg-cm-head px-2 font-normal text-white ${i === 0 ? 'text-left' : ''}`}>
              {h}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((r, i) => (
          <tr key={i} className="h-[22px] border-b border-cm-line">
            {r.map((cell, j) => (
              <td key={j} className={`px-2 tabular-nums ${j === 0 ? 'text-left text-white' : 'text-cm-orange'}`}>
                {cell}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  )
}

function Profile({ p }: { p: PersonProfile }) {
  const nav = useNav()
  const pl = p.player
  const attrs = pl?.attributes ?? p.staff?.attributes ?? {}
  const list = pl ? PLAYER_ATTRIBUTES : STAFF_ATTRIBUTES
  const items: { label: string; value: ReactNode; color?: string }[] = list.map(([key, label]) => {
    const v = attrs[key]
    return { label, value: v ?? '-', color: v ? valueColor(v) : 'text-[#777]' }
  })
  if (pl) {
    items.push(
      { label: 'Pé Preferido', value: pl.preferredFoot ?? '-' },
      { label: 'Forma', value: '-' },
      { label: 'Moral', value: pl.morale ? `${pl.morale}` : '-' },
      { label: 'Condição', value: '100%' },
    )
  }

  const born = p.birthDate ? cmDate(p.birthDate) : p.birthYear ? String(p.birthYear) : '?'
  const caps = p.internationalApps ? ` (${p.internationalApps} internac./${p.internationalGoals} golos)` : ''
  const ability = pl ?? p.staff

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="shrink-0 py-2 text-center font-title text-[21px] font-bold text-cm-yellow">
        Nascido {born} (Idade {p.age ?? '?'}).{' '}
        {p.nationId !== null && p.nation ? (
          <Link onClick={() => nav.go({ kind: 'nation', id: p.nationId! })}>{p.nation}</Link>
        ) : (
          (p.nation ?? '')
        )}
        {caps}.
      </div>
      <AttributeGrid items={items} />
      <div className="mt-2 px-1.5">
        <PurpleTable
          head={['', 'Nº', 'CA', 'PA', 'Rep. Actual', 'Rep. Mundial', 'Salário', 'Contrato até']}
          rows={[
            [
              p.club?.shortName ?? 'Sem clube',
              pl?.squadNumber ?? '-',
              ability?.currentAbility ?? '-',
              ability?.potentialAbility && ability.potentialAbility > 0 ? ability.potentialAbility : '-',
              pl?.currentReputation ?? '-',
              pl?.worldReputation ?? '-',
              p.wage ? `${money(p.wage)} p/s` : '-',
              cmDate(p.contractExpires),
            ],
          ]}
        />
      </div>
      <div className="flex-1" />
      <div className="shrink-0 py-2 text-center font-title text-[22px] font-bold text-cm-magenta">
        {pl ? pl.positionLong || '-' : (p.job ?? '-')}
      </div>
    </div>
  )
}

function Contract({ p }: { p: PersonProfile }) {
  const nav = useNav()
  const rows: [string, ReactNode][] = [
    ['Clube', p.club ? <Link key="club" onClick={() => nav.go({ kind: 'club', id: p.club!.id })}>{p.club.name}</Link> : 'Sem clube'],
    ['Função', p.job ?? '-'],
    ['No clube desde', cmDate(p.clubJoined)],
    ['Contrato até', cmDate(p.contractExpires)],
    ['Salário', p.wage ? `${money(p.wage)} por semana` : '-'],
    [
      'Nacionalidade',
      p.nation ? (
        <span key="nat">
          {[
            [p.nationId, p.nation],
            [p.secondNationId, p.secondNation],
          ]
            .filter(([, name]) => name)
            .map(([id, name], i) => (
              <span key={i}>
                {i > 0 && ' / '}
                {id !== null ? <Link onClick={() => nav.go({ kind: 'nation', id: id as number })}>{name}</Link> : name}
              </span>
            ))}
        </span>
      ) : (
        '-'
      ),
    ],
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

function History({ p }: { p: PersonProfile }) {
  const nav = useNav()
  if (p.career.length === 0) return <div className="p-6 text-center text-cm-magenta">Sem histórico registado.</div>
  return (
    <div className="min-h-0 flex-1 overflow-auto px-1.5 pt-2">
      <PurpleTable
        head={['Clube', 'Ano', 'Jogos', 'Golos']}
        rows={p.career.map((c) => [
          <span key="club" className="text-cm-magenta">
            {c.club ? <Link onClick={() => nav.go({ kind: 'club', id: c.club!.id })}>{c.club.shortName}</Link> : '-'}
            {c.onLoan && <span className="text-cm-yellow"> (emp.)</span>}
          </span>,
          c.year ?? '-',
          c.apps,
          c.goals,
        ])}
      />
    </div>
  )
}

export default function PersonPage({ id }: { id: number }) {
  const nav = useNav()
  const [person, setPerson] = useState<PersonProfile | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState('profile')

  useEffect(() => {
    api.getPerson(id).then(setPerson, (e) => setError(String(e)))
  }, [id])

  // Esc / Backspace voltam para a tela anterior
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.key === 'Escape' || e.key === 'Backspace') && !(document.activeElement instanceof HTMLInputElement)) {
        nav.back()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [nav])

  const club = person?.club
  const colours = club?.fg && club?.bg ? { fg: club.fg, bg: club.bg } : DEFAULT_COLOURS
  const number = person?.player?.squadNumber ? `${person.player.squadNumber}. ` : ''
  const title = person ? (
    <>
      {number}
      {person.name} (
      {club ? <Link onClick={() => nav.go({ kind: 'club', id: club.id })}>{club.shortName}</Link> : 'Sem clube'})
    </>
  ) : (
    ''
  )

  return (
    <div className="flex h-full flex-col">
      <TitleBand title={title} fg={colours.fg} bg={colours.bg} />
      <Tabs tabs={TABS} active={tab} onChange={setTab} />
      <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 flex-col border border-cm-line bg-cm-panel/70">
        {error && <div className="bg-red-900 px-2 py-1 text-white">{error}</div>}
        {person && tab === 'profile' && <Profile p={person} />}
        {person && tab === 'contract' && <Contract p={person} />}
        {person && tab === 'history' && <History p={person} />}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
