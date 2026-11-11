import { ChevronDown } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { PersonProfile } from '../bindings/PersonProfile'
import CascadeMenu, { type MenuNode } from '../components/CascadeMenu'
import { DialogFrame, Overlay } from '../components/Dialog'
import DisciplineDialog from '../components/DisciplineDialog'
import PersonRecords from '../components/PersonRecords'
import PlayerHistory, { SeasonTable } from '../components/PlayerHistory'
import ReportDialog from '../components/ReportDialog'
import SquadRoleDialog from '../components/SquadRoleDialog'
import { BottomBar, Empty, ErrorBox, Link, Panel, PurpleMenu, Tabs } from '../components/Screen'
import type { PositionTag } from '../bindings/PositionTag'
import type { TransferView } from '../bindings/TransferView'
import TransferActions, { type TransferAction } from '../components/TransferActions'
import TextDialog from '../components/TextDialog'
import TransferTab from '../components/TransferTab'
import { compareFirst, setCompareFirst } from '../compare'
import { personActions, takePendingAct } from '../personActions'
import { cmDate, money, shortListName, wage } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'
import { PLAYER_ATTRIBUTES } from '../attributes'

// Atributos na ordem e com os nomes do perfil do CM 01/02 em português.

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
  { key: 'profile', label: 'Perfil Geral' },
  { key: 'contract', label: 'Contrato & Transferência' },
  { key: 'history', label: 'Historial / Carreira' },
  { key: 'injuries', label: 'Lesões & Castigos' },
  { key: 'records', label: 'Marcos & Recordes' },
]

const DEFAULT_COLOURS = { fg: '#ffffff', bg: '#3a3a3a' }

/** A cor de um atributo pela faixa, como no CM: fraco, razoável, bom, excelente. */
function valueColor(v: number) {
  if (v >= 16) return 'text-[#ff3b3b]'
  if (v >= 11) return 'text-cm-orange'
  if (v >= 6) return 'text-[#f5e663]'
  return 'text-[#8f8fa0]'
}

/** Grade de 3 colunas "Nome .... valor", como no perfil do CM. */
/** `sep`: a linha abre um bloco à parte (a Forma, a Moral e a Condição depois dos atributos). */
function AttributeGrid({ items }: { items: { label: string; value: ReactNode; color?: string; sep?: boolean }[] }) {
  const perColumn = Math.ceil(items.length / 3)
  const columns = [0, 1, 2].map((c) => items.slice(c * perColumn, (c + 1) * perColumn))
  return (
    <div className="grid shrink-0 grid-cols-3 gap-2">
      {columns.map((col, c) => (
        <div key={c} className="cm-zebra overflow-hidden rounded-md bg-cm-surface">
          {col.map((it) => (
            <div
              key={it.label}
              className={`flex h-6 items-center justify-between px-3 ${it.sep ? 'mt-1.5 border-t-2 border-cm-yellow/40' : ''}`}
            >
              <span className="text-[14.5px] text-cm-text">{it.label}</span>
              <span className={`text-[14.5px] font-bold tabular-nums ${it.color ?? 'text-cm-orange'}`}>{it.value}</span>
            </div>
          ))}
        </div>
      ))}
    </div>
  )
}

/** Tabela pequena de cabeçalho roxo (a época quando não há dados do jogo). */
function PurpleTable({ head, rows }: { head: string[]; rows: ReactNode[][] }) {
  return (
    <table className="w-full overflow-hidden rounded-md border-collapse bg-cm-surface text-center">
      <thead>
        <tr className="h-8">
          {head.map((h, i) => (
            <th
              key={i}
              className={`bg-cm-headdeep px-3 font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-cm-yellow ${i === 0 ? 'text-left' : ''}`}
            >
              {h}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((r, i) => (
          <tr key={i} className="h-[28px] text-[14.5px]">
            {r.map((cell, j) => (
              <td key={j} className={`px-3 tabular-nums ${j === 0 ? 'text-left text-white' : 'text-cm-orange'}`}>
                {cell}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  )
}

/** Cabeçalho do perfil: número e clube, nome, a linha dos dados, a faixa da posição e as abas,
 * tudo num painel; Acção no canto. */
function PersonHeader({ p, action, tabs }: { p: PersonProfile | null; action: ReactNode; tabs: ReactNode }) {
  const nav = useNav()
  const club = p?.club
  const colours = club?.fg && club?.bg ? { fg: club.fg, bg: club.bg } : DEFAULT_COLOURS
  const pl = p?.player
  const born = p?.birthDate ? cmDate(p.birthDate) : p?.birthYear ? String(p.birthYear) : '?'
  const dot = <span className="text-[#5a5a66]"> · </span>
  // O ▸ do canto, como no CM: os outros jogadores do plantel do clube, por ordem alfabética
  const [squadMenu, setSquadMenu] = useState<{ x: number; y: number; items: MenuNode[] } | null>(null)
  const openSquadMenu = (e: React.MouseEvent) => {
    if (squadMenu || !club) {
      setSquadMenu(null)
      return
    }
    const r = e.currentTarget.getBoundingClientRect()
    api.getClub(club.id).then(
      (c) =>
        setSquadMenu({
          x: r.left,
          y: r.bottom + 2,
          items: c.players
            .filter((s) => !s.loanedOut)
            .sort((a, b) => a.listName.localeCompare(b.listName))
            .map((s) => ({ label: shortListName(s.listName), onSelect: () => nav.go({ kind: 'person', id: s.id }) })),
        }),
      () => setSquadMenu(null),
    )
  }
  return (
    <div className="m-2 shrink-0 rounded-md bg-cm-surface px-3 pb-1.5 pt-2">
      <div className="flex items-start gap-3">
        {club && (
          <button
            data-menu-anchor
            className="cm-purple flex h-[44px] w-6 shrink-0 items-center justify-center text-[13px]"
            title="Plantel do clube"
            aria-expanded={!!squadMenu}
            onClick={openSquadMenu}
          >
            ▸
          </button>
        )}
        <div
          className="flex size-[44px] shrink-0 flex-col items-center justify-center rounded border border-black/40 leading-none"
          style={{ color: colours.fg, background: colours.bg }}
          title={club?.name}
        >
          <span className="font-title text-[22px] font-bold">{pl?.squadNumber ?? '-'}</span>
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate font-title text-[22px] font-bold uppercase leading-tight text-white">{p?.name ?? ''}</span>
            {p && (
              <span className="shrink-0 rounded px-1.5 font-mono text-[11px] font-bold uppercase tracking-wider" style={{ color: colours.fg, background: colours.bg }}>
                {club ? (
                  <Link onClick={() => nav.go({ kind: 'club', id: club.id, reserves: p.status?.reserve })}>
                    {p.status?.reserve ? `Reservas ${club.shortName}` : club.shortName}
                  </Link>
                ) : (
                  'Sem clube'
                )}
              </span>
            )}
            {p?.ghost && (
              <span className="shrink-0 rounded bg-cm-headdeep px-1.5 font-mono text-[11px] font-bold uppercase tracking-wider text-cm-purplelight">
                Enchimento
              </span>
            )}
            {p && !pl && p.job && (
              <span className="shrink-0 rounded bg-cm-dlgdeep px-1.5 font-mono text-[11px] font-bold uppercase tracking-wider text-cm-mint">{p.job}</span>
            )}
          </div>
          {p && (
            <div className="truncate font-mono text-[13px] text-cm-muted">
              {born} ({p.age ?? '?'} anos)
              {dot}
              {p.nationId !== null && p.nation ? <Link onClick={() => nav.go({ kind: 'nation', id: p.nationId! })}>{p.nation}</Link> : (p.nation ?? '-')}
              {p.internationalApps > 0 && (
                <>
                  {' '}
                  (<b className="text-cm-yellow">{p.internationalApps}</b> internac. / <b className="text-cm-yellow">{p.internationalGoals}</b> golos)
                </>
              )}
              {pl && !p.ghost && (
                <>
                  {dot}Valor: <b className="text-cm-mint">{money(pl.value) || '-'}</b>
                </>
              )}
              {!p.ghost && (
                <>
                  {dot}Salário: <b className="text-white">{wage(p.wage) || '-'}</b>
                </>
              )}
            </div>
          )}
        </div>
        <div className="shrink-0">{action}</div>
      </div>
      {pl && (
        <div className="mt-1.5 flex h-7 items-center gap-2 rounded bg-black/30 px-3">
          <span className="truncate font-title text-[16px] font-bold text-cm-mint">{pl.positionLong || '-'}</span>
          <span className="ml-auto flex shrink-0 gap-1">
            {pl.positions.map((t) => (
              <span
                key={t.code}
                title={t.natural ? 'Posição de origem' : undefined}
                className={`rounded px-1.5 font-mono text-[11px] font-bold tracking-wider ${t.natural ? 'bg-cm-dlgborder text-black' : 'bg-cm-dlgdeep text-cm-mint'}`}
              >
                {t.code}
              </span>
            ))}
          </span>
          <span className="shrink-0 font-mono text-[11px] uppercase tracking-wider text-cm-dim">
            Pé preferido: <b className="text-cm-yellow">{pl.preferredFoot ?? '-'}</b>
          </span>
        </div>
      )}
      <div className="mt-1.5">{tabs}</div>
      {squadMenu && <CascadeMenu x={squadMenu.x} y={squadMenu.y} items={squadMenu.items} onClose={() => setSquadMenu(null)} />}
    </div>
  )
}

// Onde fica cada posição no campo pequeno (ataque para a direita, esquerda em cima)
const ROLE_X: Record<string, number> = { GR: 6, L: 16, D: 25, AL: 40, MD: 42, M: 57, MA: 72, A: 88 }
const SIDE_Y: Record<string, number> = { E: 18, C: 50, D: 82 }

/** Campo pequeno com as posições em que joga (a de origem a cheio). */
function Pitch({ tags }: { tags: PositionTag[] }) {
  return (
    <div className="relative h-[118px] overflow-hidden rounded border border-cm-dlgline bg-cm-dlg">
      <div className="absolute inset-y-0 left-1/2 border-l border-cm-dlgline" />
      <div className="absolute left-1/2 top-1/2 size-9 -translate-x-1/2 -translate-y-1/2 rounded-full border border-cm-dlgline" />
      <div className="absolute left-0 top-1/2 h-12 w-5 -translate-y-1/2 border border-l-0 border-cm-dlgline" />
      <div className="absolute right-0 top-1/2 h-12 w-5 -translate-y-1/2 border border-r-0 border-cm-dlgline" />
      {tags.map((t) => (
        <span
          key={t.code}
          className={`absolute -translate-x-1/2 -translate-y-1/2 rounded px-1 font-mono text-[10px] font-bold ${
            t.natural ? 'bg-cm-dlgborder text-black' : 'border border-cm-dlgborder bg-cm-dlg text-cm-mint'
          }`}
          style={{ left: `${ROLE_X[t.role] ?? 50}%`, top: `${SIDE_Y[t.side ?? 'C']}%` }}
        >
          {t.code}
        </span>
      ))}
    </div>
  )
}

/** Linhas "Nome .... valor" de um painel. */
function InfoRows({ rows }: { rows: [string, ReactNode][] }) {
  return (
    <div className="cm-zebra">
      {rows.map(([k, v]) => (
        <div key={k} className="flex h-6 items-center justify-between gap-2 px-3 text-[13.5px]">
          <span className="text-cm-muted">{k}</span>
          <span className="truncate font-bold text-cm-yellow">{v}</span>
        </div>
      ))}
    </div>
  )
}

function Profile({ p }: { p: PersonProfile }) {
  const pl = p.player
  const attrs = pl?.attributes ?? p.staff?.attributes ?? {}
  const list = pl ? PLAYER_ATTRIBUTES : STAFF_ATTRIBUTES
  const items: { label: string; value: ReactNode; color?: string; sep?: boolean }[] = list.map(([key, label]) => {
    const v = attrs[key]
    return { label, value: v ?? '-', color: v ? valueColor(v) : 'text-[#777]' }
  })
  if (pl) {
    items.push(
      { label: 'Forma', sep: true, value: p.form.length ? p.form.map((r) => Math.round(r)).join('-') : '-' },
      { label: 'Moral', value: p.status?.moraleLabel ?? '-' },
      { label: 'Condição', value: `${p.status?.condition ?? 100}%` },
    )
  }
  const ability = pl ?? p.staff
  const total = p.season.reduce((t, s) => ({ apps: t.apps + s.apps, goals: t.goals + s.goals, assists: t.assists + s.assists }), {
    apps: 0,
    goals: 0,
    assists: 0,
  })
  const rating = total.apps ? p.season.reduce((t, s) => t + s.rating * s.apps, 0) / total.apps : 0

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-auto pb-1">
      <div className="flex shrink-0 gap-2">
        <div className="min-w-0 flex-1">
          <AttributeGrid items={items} />
        </div>
        <div className="flex w-[250px] shrink-0 flex-col gap-2">
          {pl && (
            <Panel title="Posições">
              <div className="p-2">
                <Pitch tags={pl.positions} />
              </div>
            </Panel>
          )}
          {p.assistantNote && (
            <Panel title="Relatório do Adjunto">
              <div className="px-3 py-2 text-[13.5px] italic leading-snug text-white">“{p.assistantNote}”</div>
            </Panel>
          )}
          <Panel title="Situação">
            <InfoRows
              rows={[
                ...(pl ? ([['Número', pl.squadNumber ?? '-']] as [string, ReactNode][]) : []),
                ['Rep. Actual', ability?.currentReputation ?? '-'],
                ['Rep. Mundial', ability?.worldReputation ?? '-'],
                ['No clube desde', cmDate(p.clubJoined)],
                ['Contrato até', cmDate(p.contractExpires)],
              ]}
            />
            {p.note && (
              <div className="border-t border-cm-line px-3 py-1.5 text-[13px] text-white">
                <span className="text-cm-yellow">Nota: </span>
                {p.note}
              </div>
            )}
          </Panel>
        </div>
      </div>
      {p.knowledge && p.knowledge !== 'known' && (
        <div className="shrink-0 rounded border border-cm-line bg-cm-panel px-3 py-1 text-center text-cm-yellow">
          {p.reportOn
            ? `Os seus olheiros estão a observá-lo: o relatório chega a ${cmDate(p.reportOn)}.`
            : p.knowledge === 'unknown'
              ? 'O seu clube não conhece este jogador. Mande observá-lo (Acção → Observar Jogador).'
              : 'O seu clube conhece este jogador só em parte. Mande observá-lo para ver tudo.'}
        </div>
      )}
      {pl && p.seasons[0]?.byKind.length ? (
        <Panel title={`Estatísticas da Época ${p.seasons[0].label}`} className="shrink-0">
          <SeasonTable byKind={p.seasons[0].byKind} />
        </Panel>
      ) : (
        pl && (
          <Panel title="Estatísticas da Época" className="shrink-0">
            {p.season.length === 0 ? (
              <Empty className="p-3">Ainda não jogou esta época.</Empty>
            ) : (
              <PurpleTable
                head={['Competição', 'Jogos', 'Golos', 'Asts', 'Nota']}
                rows={[
                  ...p.season.map((s) => [s.competition, s.apps, s.goals, s.assists, s.rating.toFixed(2)]),
                  [<b key="t" className="text-cm-mint">Total</b>, total.apps, total.goals, total.assists, rating.toFixed(2)],
                ]}
              />
            )}
          </Panel>
        )
      )}
    </div>
  )
}

/** Aba "Lesões & Castigos": só num jogo em curso, para jogadores de clubes vivos. */
/** Informação pessoal, como a do FM08: preferências, personalidade, felicidade e relações. */
function PersonalPanel({ p }: { p: PersonProfile }) {
  const nav = useNav()
  const info = p.personal!
  const clubs = (list: typeof info.favouredClubs) =>
    list.length
      ? list.map((c, i) => (
          <span key={c.id}>
            {i > 0 && ', '}
            <Link onClick={() => nav.go({ kind: 'club', id: c.id })}>{c.shortName}</Link>
          </span>
        ))
      : '-'
  const people = (list: typeof info.favouredPeople) =>
    list.length
      ? list.map((x, i) => (
          <span key={x.id}>
            {i > 0 && ', '}
            <Link onClick={() => nav.go({ kind: 'person', id: x.id })}>{x.name}</Link>
          </span>
        ))
      : '-'
  const rows: [string, ReactNode][] = [
    ['Personalidade', info.personality],
    ['Reputação', info.reputation],
    ['Clubes favoritos', clubs(info.favouredClubs)],
    ['Clubes detestados', clubs(info.dislikedClubs)],
    ['Pessoas favoritas', people(info.favouredPeople)],
    ['Pessoas detestadas', people(info.dislikedPeople)],
    ['Felicidade', info.happiness],
    ['Planos', info.plans],
    ['Relações', info.relationships.length ? info.relationships.join('; ') : '-'],
  ]
  return (
    <Panel title="Informação Pessoal">
      <div className="cm-zebra">
        {rows.map(([k, v]) => (
          <div key={k} className="flex min-h-6 items-start gap-3 px-3 py-0.5 text-[13.5px]">
            <span className="w-36 shrink-0 text-cm-muted">{k}</span>
            <span className="min-w-0 flex-1 text-right font-bold text-cm-yellow">{v}</span>
          </div>
        ))}
      </div>
    </Panel>
  )
}

function Injuries({ p }: { p: PersonProfile }) {
  const st = p.status
  if (!st) return <Empty>Sem registos.</Empty>
  const health: [string, ReactNode][] = [
    ['Condição', `${st.condition}%`],
    [
      'Lesão',
      st.injury ? (
        <span key="i" className="text-cm-red">
          {st.injury}
        </span>
      ) : (
        'Nenhuma'
      ),
    ],
    ['Regresso previsto', st.injuryUntil ? cmDate(st.injuryUntil) : '-'],
    ['Moral', st.moraleLabel],
  ]
  const rows: [string, ReactNode][] = [
    [
      'Suspensão',
      st.suspended ? (
        <span key="s" className="text-cm-red">
          {`${st.suspended} jogo${st.suspended > 1 ? 's' : ''}`}
        </span>
      ) : (
        'Nenhuma'
      ),
    ],
    ['Cartões amarelos', `${st.yellows} (suspensão ao 3º)`],
    // Cup-tied: já jogou a continental por outro clube nesta época
    [
      'Continentais',
      st.cupTied.length ? (
        <span key="c" className="text-cm-red">
          {`Não joga: ${st.cupTied.join(', ')}`}
        </span>
      ) : (
        'Pode jogar'
      ),
    ],
  ]
  return (
    <div className="flex min-h-0 flex-1 items-start gap-2 overflow-auto pb-1">
      <Panel title="Condição Física" chip={st.injury ? undefined : 'Apto para jogar'} className="min-w-0 flex-1">
        <InfoRows rows={health} />
      </Panel>
      <Panel title="Disciplina & Suspensões" className="min-w-0 flex-1">
        <InfoRows rows={rows} />
      </Panel>
    </div>
  )
}

/** " (3 anos)": o que falta do contrato, contado a partir da data do jogo. */
function useYearsLeft() {
  const today = useGame().info?.date
  return (until: string) => {
    if (!today) return ''
    const y = Number(until.slice(0, 4)) - Number(today.slice(0, 4)) - (until.slice(5) < today.slice(5) ? 1 : 0)
    return y >= 1 ? ` (${y} ano${y > 1 ? 's' : ''})` : ' (último ano)'
  }
}

function Contract({ p }: { p: PersonProfile }) {
  const years = useYearsLeft()
  const nav = useNav()
  const rows: [string, ReactNode][] = [
    ['Clube', p.club ? <Link key="club" onClick={() => nav.go({ kind: 'club', id: p.club!.id })}>{p.club.name}</Link> : 'Sem clube'],
    ['Função', p.job ?? '-'],
    ['No clube desde', cmDate(p.clubJoined)],
    ['Contrato até', p.contractExpires ? `${cmDate(p.contractExpires)}${years(p.contractExpires)}` : '-'],
    ['Salário', wage(p.wage, true)],
    ...(p.player ? ([['Valor', money(p.player.value) || '-']] as [string, ReactNode][]) : []),
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
  return <InfoRows rows={rows} />
}

export default function PersonPage({ id, tab: startTab }: { id: number; tab?: string }) {
  const nav = useNav()
  const game = useGame()
  const [person, setPerson] = useState<PersonProfile | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState(startTab ?? 'profile')
  const [confirm, setConfirm] = useState(false)
  const [report, setReport] = useState<'medical' | 'fitness' | null>(null)
  const [discipline, setDiscipline] = useState(false)
  const [roleDialog, setRoleDialog] = useState(false)
  const [notice, setNotice] = useState<string | null>(null)
  // A situação de mercado do jogador (para o menu Acção) e o diálogo aberto
  const [market, setMarket] = useState<TransferView | null>(null)
  const [deal, setDeal] = useState<TransferAction | null>(null)
  const [version, setVersion] = useState(0)
  const [text, setText] = useState<'nickname' | 'note' | null>(null)
  const [release, setRelease] = useState<string | null>(null)

  const reload = () => {
    setVersion((v) => v + 1)
    return api.getPerson(id).then(setPerson, (e) => setError(String(e)))
  }
  useEffect(() => {
    api.getPerson(id).then(setPerson, (e) => setError(String(e)))
  }, [id])
  const playing = game.mode === 'game' && !!person?.player
  const date = game.info?.date
  useEffect(() => {
    if (playing) api.transferView(id).then(setMarket, () => setMarket(null))
  }, [id, playing, version, date])

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

  // Menu Acção (num jogo em curso): de outro clube (ou livre), propor contrato e observar; do
  // plantel, disciplina, imprensa e relatórios do staff. Quem pode assinar decide o core.
  const mine = !!person && person.club?.id === game.info?.club.id
  const m = market
  const groups = person ? personActions({ person, market, mine, playing: game.mode === 'game' }) : []
  // O último grupo é o de todos (alcunha, nota, comparar)
  const actions = groups.slice(0, -1)
  // Os botões da aba Contrato & Transferência: os principais, iguais aos do menu
  const pick = (keys: string[]) => actions.flat().filter((a) => keys.includes(a.key))
  const contractButtons = pick(['renew', 'role', 'contract', 'recall'])
  const marketButtons = pick(['list', 'unlist', 'price', 'loanlist', 'unloanlist', 'shop', 'offer', 'loan', 'option', 'shortlist', 'unshortlist', 'scout'])
  const buttonRow = (list: { key: string; label: string }[]) =>
    list.length > 0 && (
      <div className="flex flex-wrap gap-1.5 border-t border-cm-line p-2">
        {list.map((b) => (
          <button key={b.key} className="cm-purple h-7 px-3" onClick={() => act(b.key)}>
            {b.label}
          </button>
        ))}
      </div>
    )
  const first = compareFirst()
  // As mensagens novas contam no "por ler"
  const refreshInfo = () => void api.gameInfo().then((info) => info && game.update(info))
  // Uma acção que deixa uma mensagem volta às Notícias, como no CM (a mensagem nova abre lá)
  const toNews = () => {
    refreshInfo()
    nav.go({ kind: 'news', latest: true })
  }
  const comment = async (kind: 'praise' | 'criticise') => {
    try {
      await api.pressComment(id, kind)
      toNews()
    } catch (e) {
      setError(String(e))
    }
  }

  const moveSquad = async (reserve: boolean) => {
    try {
      setNotice(await api.setReserve(id, reserve))
      setError(null)
      await reload()
    } catch (e) {
      setError(String(e))
    }
  }

  const giveLeave = async () => {
    try {
      setNotice(await api.giveLeave(id))
      setError(null)
      await reload()
    } catch (e) {
      setError(String(e))
    }
  }

  // Uma acção de mercado do menu: mostra a aba Transferência com o resultado
  const marketAction = async (f: () => Promise<TransferView>) => {
    try {
      setMarket(await f())
      setError(null)
      setTab('contract')
      refreshInfo()
      await reload()
    } catch (e) {
      setError(String(e))
    }
  }

  const scout = async () => {
    setConfirm(false)
    try {
      setNotice(await api.scoutPlayer(id))
      setError(null)
      await reload()
    } catch (e) {
      setError(String(e))
    }
  }

  const act = (k: string) => {
    if (k === 'contract' || k === 'renew') nav.go({ kind: 'contract', id })
    else if (k === 'offer' || k === 'loan' || k === 'price' || k === 'shop') setDeal(k)
    // A opção de compra deixa a mensagem com o contrato a negociar: volta às Notícias
    else if (k === 'option') void api.exerciseOption(id).then(toNews, (e) => setError(String(e)))
    else if (k === 'nickname' || k === 'note') setText(k)
    else if (k === 'release') void api.releaseQuestion(id).then(setRelease, (e) => setError(String(e)))
    else if (k === 'compareFirst' && person) {
      setCompareFirst({ id, name: person.name })
      setNotice(`Escolha o segundo jogador e, no menu Acção, Comparar com ${person.name}.`)
    } else if (k === 'compare' && first) {
      setCompareFirst(null)
      nav.go({ kind: 'compare', a: first.id, b: id })
    }
    else if (k === 'list' || k === 'unlist' || k === 'loanlist' || k === 'unloanlist')
      void marketAction(() =>
        api.setListing(
          id,
          k === 'list' ? true : k === 'unlist' ? false : !!m?.transferListed,
          k === 'loanlist' ? true : k === 'unloanlist' ? false : !!m?.loanListed,
          m?.asking ?? null,
        ),
      )
    else if (k === 'shortlist' || k === 'unshortlist')
      void api.setShortlist(id, k === 'shortlist').then(() => {
        setNotice(k === 'shortlist' ? 'Adicionado à Lista Preferencial.' : 'Retirado da Lista Preferencial.')
        void reload()
      })
    else if (k === 'scout') setConfirm(true)
    else if (k === 'discipline') setDiscipline(true)
    else if (k === 'role') setRoleDialog(true)
    else if (k === 'praise' || k === 'criticise') void comment(k)
    else if (k === 'promote' || k === 'demote') void moveSquad(k === 'demote')
    else if (k === 'recall')
      void api.recallLoan(id).then(
        (text) => {
          setNotice(text)
          setError(null)
          void reload()
        },
        (e) => setError(String(e)),
      )
    else if (k === 'leave') void giveLeave()
    else setReport(k as 'medical' | 'fitness')
  }

  // A acção escolhida no botão direito de uma lista: corre quando o perfil (e, num jogo, o
  // mercado) chegam
  const ready = !!person && (!playing || !!market)
  useEffect(() => {
    if (!ready) return
    const k = takePendingAct(id)
    if (k) act(k)
    // Só ao abrir: `act` muda a cada render
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ready, id])

  return (
    <div className="relative flex h-full flex-col">
      <PersonHeader
        p={person}
        action={
          game.mode === 'game' &&
          person?.player &&
          // O jogador de enchimento só tem os atributos, como no CM
          !person.ghost && (
            <PurpleMenu
              label="Acção"
              align="right"
              buttonClass="flex h-9 items-center gap-1.5 rounded bg-cm-orange px-4 font-title text-[17px] font-bold text-black hover:brightness-110"
              trigger={
                <>
                  Acção <ChevronDown size={14} />
                </>
              }
              options={groups.flatMap((g, i) => (i ? ['separator' as const, ...g] : g))}
              onChange={act}
            />
          )
        }
        tabs={
          <Tabs
            tabs={(person?.player && game.mode === 'game' ? TABS : TABS.filter((t) => t.key !== 'injuries' && t.key !== 'records')).map((t) =>
              person?.ghost && t.key !== 'profile'
                ? { ...t, disabled: true }
                : t.key === 'injuries'
                  ? { ...t, label: person?.status?.injury || person?.status?.suspended ? `${t.label} •` : t.label, disabled: !person?.status }
                  : t.key === 'contract' && !person?.player
                    ? { ...t, label: 'Contrato' }
                    : t,
            )}
            active={person?.ghost ? 'profile' : tab}
            onChange={setTab}
          />
        }
      />
      <div className="mx-2 flex min-h-0 flex-1 flex-col">
        <ErrorBox className="mb-2">{error}</ErrorBox>
        {notice && <div className="mb-2 rounded bg-cm-dlgdeep px-2 py-1 text-center text-cm-yellow">{notice}</div>}
        {person && (tab === 'profile' || person.ghost) && <Profile p={person} />}
        {person && !person.ghost && tab === 'contract' && (
          <div className="flex min-h-0 flex-1 items-start gap-2 overflow-auto pb-1">
            <div className="flex w-[42%] shrink-0 flex-col gap-2">
              <Panel title="Contrato">
                <Contract p={person} />
                {game.mode === 'game' && buttonRow(contractButtons)}
              </Panel>
              {person.personal && <PersonalPanel p={person} />}
            </div>
            {game.mode === 'game' && person.player && (
              <div className="flex min-w-0 flex-1 flex-col gap-2">
                <Panel title="Mercado">
                  <TransferTab personId={id} version={version} onChanged={() => void reload()} />
                  {buttonRow(marketButtons)}
                </Panel>
                {!!person.status?.unhappy.length && (
                  <Panel title="Estado de Espírito" chip="Insatisfeito">
                    <div className="cm-zebra">
                      {person.status.unhappy.map((u) => (
                        <div key={u} className="flex min-h-6 items-center px-3 text-[13.5px] text-cm-orange">
                          {u}.
                        </div>
                      ))}
                    </div>
                  </Panel>
                )}
                {!!person.status?.clubUnhappy.length && (
                  <Panel title="Descontentamento com o Clube">
                    <div className="cm-zebra">
                      {person.status.clubUnhappy.map((u) => (
                        <div key={u} className="flex min-h-6 items-center px-3 text-[13.5px] text-cm-orange">
                          {u}.
                        </div>
                      ))}
                    </div>
                  </Panel>
                )}
              </div>
            )}
          </div>
        )}
        {person && !person.ghost && tab === 'injuries' && <Injuries p={person} />}
        {person && !person.ghost && tab === 'history' && <PlayerHistory p={person} />}
        {person && !person.ghost && tab === 'records' && <PersonRecords p={person} />}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
      {deal && market && person && (
        <TransferActions
          action={deal}
          view={market}
          name={person.name}
          onClose={() => setDeal(null)}
          onDone={(v, message) => {
            setDeal(null)
            setMarket(v)
            // Uma proposta enviada volta às Notícias, como no CM (a resposta chega lá); o preço pedido
            // e o Oferecer a Clubes (as respostas chegam dias depois) ficam aqui
            if (deal === 'shop') {
              setNotice(message ?? null)
              setError(null)
            } else if (deal === 'price') {
              setTab('contract')
              void reload()
            } else nav.go({ kind: 'news', latest: true })
          }}
        />
      )}
      {roleDialog && (
        <SquadRoleDialog
          personId={id}
          onCancel={() => setRoleDialog(false)}
          onDone={(message) => {
            setRoleDialog(false)
            setNotice(message)
            setError(null)
            void reload()
          }}
        />
      )}
      {text && person && (
        <TextDialog
          title={text === 'nickname' ? 'Definir Alcunha' : 'Criar Nota Treinador'}
          initial={(text === 'nickname' ? person.nickname : person.note) ?? ''}
          multiline={text === 'note'}
          onCancel={() => setText(null)}
          onOk={(t) => {
            const save = text === 'nickname' ? api.setNickname(id, t) : api.setNote(id, t)
            setText(null)
            void save.then(
              () => reload(),
              (e) => setError(String(e)),
            )
          }}
        />
      )}
      {release && (
        <Overlay>
          <DialogFrame
            title="Dispensar"
            width={560}
            cancelLabel="Não"
            okLabel="Sim"
            onCancel={() => setRelease(null)}
            onOk={() => {
              setRelease(null)
              void api.releasePlayer(id).then(toNews, (e) => setError(String(e)))
            }}
          >
            <div className="py-6 text-center font-title text-[19px] text-white">{release}</div>
          </DialogFrame>
        </Overlay>
      )}
      {discipline && person && (
        <DisciplineDialog
          personId={id}
          name={person.name}
          onClose={() => setDiscipline(false)}
          onDone={() => {
            setDiscipline(false)
            toNews()
          }}
        />
      )}
      {report && person && (
        <ReportDialog
          kind={report}
          personId={id}
          name={person.name}
          onClose={() => setReport(null)}
          onDone={() => {
            setReport(null)
            toNews()
          }}
        />
      )}
      {confirm && person && (
        <Overlay>
          <DialogFrame
            title={game.info?.managerName ?? ''}
            width={520}
            cancelLabel="Não"
            okLabel="Sim"
            onCancel={() => setConfirm(false)}
            onOk={() => void scout()}
          >
            <div className="py-6 text-center font-title text-[19px] text-white">Ter {person.name} a ser observado pelos seus olheiros?</div>
          </DialogFrame>
        </Overlay>
      )}
    </div>
  )
}
