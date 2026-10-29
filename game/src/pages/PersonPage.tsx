import { ChevronDown } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { PersonProfile } from '../bindings/PersonProfile'
import { DialogFrame, Overlay } from '../components/Dialog'
import DisciplineDialog from '../components/DisciplineDialog'
import ReportDialog from '../components/ReportDialog'
import { BottomBar, Empty, ErrorBox, Link, PurpleMenu, ScreenHeader, Tabs } from '../components/Screen'
import type { TransferView } from '../bindings/TransferView'
import TransferActions, { type TransferAction } from '../components/TransferActions'
import TextDialog from '../components/TextDialog'
import TransferTab from '../components/TransferTab'
import { compareFirst, setCompareFirst } from '../compare'
import { cmDate, initials, money, wage } from '../format'
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
  { key: 'profile', label: 'Perfil' },
  { key: 'injuries', label: 'Lesões & Castigos', disabled: true },
  { key: 'contract', label: 'Contrato' },
  { key: 'transfer', label: 'Transferência' },
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
    <div className="grid shrink-0 grid-cols-3 gap-2.5">
      {columns.map((col, c) => (
        <div key={c} className="cm-zebra overflow-hidden rounded-md bg-[#1a1b21]">
          {col.map((it) => (
            <div key={it.label} className="flex h-[27px] items-center justify-between px-3">
              <span className="text-[14.5px] text-[#e3e1e9]">{it.label}</span>
              <span className={`text-[14.5px] font-bold tabular-nums ${it.color ?? 'text-cm-orange'}`}>{it.value}</span>
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
    <table className="w-full overflow-hidden rounded-md border-collapse bg-[#1a1b21] text-center">
      <thead>
        <tr className="h-8">
          {head.map((h, i) => (
            <th
              key={i}
              className={`bg-[#3d1a73] px-3 font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-cm-yellow ${i === 0 ? 'text-left' : ''}`}
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
      { label: 'Forma', value: p.form.length ? p.form.map((r) => Math.round(r)).join('-') : '-' },
      { label: 'Moral', value: p.status?.moraleLabel ?? '-' },
      { label: 'Condição', value: `${p.status?.condition ?? 100}%` },
    )
  }

  const born = p.birthDate ? cmDate(p.birthDate) : p.birthYear ? String(p.birthYear) : '?'
  const caps = p.internationalApps ? ` (${p.internationalApps} internac./${p.internationalGoals} golos)` : ''
  const ability = pl ?? p.staff

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2.5 overflow-auto">
      <div className="shrink-0 text-[16px] font-semibold text-cm-yellow">
        Nascido {born} (Idade {p.age ?? '?'}).{' '}
        {p.nationId !== null && p.nation ? (
          <Link onClick={() => nav.go({ kind: 'nation', id: p.nationId! })}>{p.nation}</Link>
        ) : (
          (p.nation ?? '')
        )}
        {caps}.
      </div>
      <AttributeGrid items={items} />
      <div className="shrink-0">
        <PurpleTable
          head={['', 'Nº', 'Rep. Actual', 'Rep. Mundial', 'Salário', 'Contrato até']}
          rows={[
            [
              p.club?.shortName ?? 'Sem clube',
              pl?.squadNumber ?? '-',
              ability?.currentReputation ?? '-',
              ability?.worldReputation ?? '-',
              wage(p.wage),
              cmDate(p.contractExpires),
            ],
          ]}
        />
      </div>
      {p.season.length > 0 && (
        <div className="shrink-0">
          <PurpleTable
            head={['Época', 'Jogos', 'Golos', 'Asts', 'Nota']}
            rows={p.season.map((s) => [s.competition, s.apps, s.goals, s.assists, s.rating.toFixed(2)])}
          />
        </div>
      )}
      {p.note && (
        <div className="shrink-0 rounded-md bg-[#1a1b21] px-3 py-1.5 text-[15px] text-white">
          <span className="text-cm-yellow">Nota do treinador: </span>
          {p.note}
        </div>
      )}
      {p.knowledge && p.knowledge !== 'known' && (
        <div className="shrink-0 text-center text-cm-magenta">
          {p.reportOn
            ? `Os seus olheiros estão a observá-lo: o relatório chega a ${cmDate(p.reportOn)}.`
            : p.knowledge === 'unknown'
              ? 'O seu clube não conhece este jogador. Mande observá-lo (Acção → Observar Jogador).'
              : 'O seu clube conhece este jogador só em parte. Mande observá-lo para ver tudo.'}
        </div>
      )}
      {!!p.status?.unhappy.length && (
        <div className="shrink-0 text-center text-cm-orange">
          {p.status.unhappy.map((u) => (
            <div key={u}>{u}.</div>
          ))}
        </div>
      )}
      <div className="flex-1" />
      <div className="shrink-0 text-center font-title text-[20px] font-bold text-cm-magenta">
        {pl ? pl.positionLong || '-' : (p.job ?? '-')}
      </div>
    </div>
  )
}

/** Aba "Lesões & Castigos": só num jogo em curso, para jogadores de clubes vivos. */
function Injuries({ p }: { p: PersonProfile }) {
  const st = p.status
  if (!st) return <Empty>Sem registos.</Empty>
  const rows: [string, ReactNode][] = [
    ['Lesão', st.injury ?? 'Nenhuma'],
    ['Regresso previsto', st.injuryUntil ? cmDate(st.injuryUntil) : '-'],
    ['Suspensão', st.suspended ? `${st.suspended} jogo${st.suspended > 1 ? 's' : ''}` : 'Nenhuma'],
    ['Cartões amarelos', `${st.yellows} (suspensão ao 3º)`],
    ['Condição', `${st.condition}%`],
    ['Moral', st.moraleLabel],
  ]
  return (
    <div className="cm-zebra mx-auto mt-2 w-full max-w-2xl overflow-hidden rounded-md bg-[#1a1b21]">
      {rows.map(([k, v]) => (
        <div key={k} className="flex h-[32px] items-center justify-between px-4 text-[15px]">
          <span className="text-white">{k}</span>
          <span className={`font-bold ${k === 'Lesão' && st.injury ? 'text-[#ff5050]' : 'text-cm-yellow'}`}>{v}</span>
        </div>
      ))}
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
  return (
    <div className="cm-zebra mx-auto mt-2 w-full max-w-2xl overflow-hidden rounded-md bg-[#1a1b21]">
      {rows.map(([k, v]) => (
        <div key={k} className="flex h-[32px] items-center justify-between px-4 text-[15px]">
          <span className="text-white">{k}</span>
          <span className="font-bold text-cm-yellow">{v}</span>
        </div>
      ))}
    </div>
  )
}

function History({ p }: { p: PersonProfile }) {
  const nav = useNav()
  if (p.career.length === 0) return <Empty>Sem histórico registado.</Empty>
  return (
    <div className="min-h-0 flex-1 overflow-auto">
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
  const game = useGame()
  const [person, setPerson] = useState<PersonProfile | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState('profile')
  const [confirm, setConfirm] = useState(false)
  const [report, setReport] = useState<'medical' | 'fitness' | null>(null)
  const [discipline, setDiscipline] = useState(false)
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

  const club = person?.club
  const colours = club?.fg && club?.bg ? { fg: club.fg, bg: club.bg } : DEFAULT_COLOURS
  const number = person?.player?.squadNumber ? `${person.player.squadNumber}. ` : ''
  const title = person ? `${number}${person.name}` : ''
  const clubTag = person ? (
    <span className="rounded px-1.5 tracking-wider" style={{ color: colours.fg, background: colours.bg }}>
      {club ? (
        <Link onClick={() => nav.go({ kind: 'club', id: club.id, reserves: person.status?.reserve })}>
          {person.status?.reserve ? `Reservas ${club.shortName}` : club.shortName}
        </Link>
      ) : (
        'Sem clube'
      )}
    </span>
  ) : null

  // Menu Acção (num jogo em curso): de outro clube (ou livre), propor contrato e observar; do
  // plantel, disciplina, imprensa e relatórios do staff. Quem pode assinar decide o core.
  const mine = !!person && person.club?.id === game.info?.club.id
  const canScout = game.mode === 'game' && !!person?.player && !mine && !person.reportOn
  const m = market
  const actions: { key: string; label: string }[][] = mine
    ? [
        // Mercado
        [
          ...(m?.canRenew ? [{ key: 'renew', label: 'Oferecer Novo Contrato' }] : []),
          m?.transferListed
            ? { key: 'unlist', label: 'Retirar da Lista de Transferências' }
            : { key: 'list', label: 'Colocar na Lista de Transferências' },
          m?.loanListed
            ? { key: 'unloanlist', label: 'Retirar da Lista de Empréstimos' }
            : { key: 'loanlist', label: 'Colocar na Lista de Empréstimos' },
          ...(m?.transferListed ? [{ key: 'price', label: 'Definir Preço Pedido' }] : []),
        ],
        // Plantel
        [
          person?.status?.reserve
            ? { key: 'promote', label: 'Promover ao Plantel Principal' }
            : { key: 'demote', label: 'Despromover para Reservas' },
          { key: 'discipline', label: 'Disciplinar Jogador' },
          { key: 'release', label: 'Dispensar' },
        ],
        // Imprensa
        [
          { key: 'praise', label: 'Elogiar na Imprensa' },
          { key: 'criticise', label: 'Criticar na Imprensa' },
        ],
        // Staff
        [
          { key: 'medical', label: 'Pedir Relatório Médico' },
          { key: 'fitness', label: 'Pedir Relatório Preparador Físico' },
        ],
      ]
    : [
        [
          ...(m?.canOffer && !m.free ? [{ key: 'offer', label: 'Abordar Para Comprar' }] : []),
          ...(m?.canLoan ? [{ key: 'loan', label: 'Propôr Empréstimo' }] : []),
          ...(m?.canOption ? [{ key: 'option', label: 'Exercer Opção de Compra' }] : []),
          ...(person?.canContract ? [{ key: 'contract', label: 'Propor Contrato' }] : []),
        ],
        [
          person?.shortlisted
            ? { key: 'unshortlist', label: 'Retirar da Lista Preferencial' }
            : { key: 'shortlist', label: 'Adicionar à Lista Preferencial' },
          ...(canScout ? [{ key: 'scout', label: 'Observar Jogador' }] : []),
        ],
      ]
  // Para todos: alcunha, nota e comparar (o 1º escolhido espera pelo 2º)
  const first = compareFirst()
  const common: { key: string; label: string }[] = [
    { key: 'nickname', label: 'Definir Alcunha' },
    { key: 'note', label: 'Criar Nota Treinador' },
    first && first.id !== id ? { key: 'compare', label: `Comparar com ${first.name}` } : { key: 'compareFirst', label: 'Comparar Jogadores' },
  ]
  // As mensagens novas contam no "por ler"
  const refreshInfo = () => void api.gameInfo().then((info) => info && game.update(info))
  const comment = async (kind: 'praise' | 'criticise') => {
    try {
      setNotice(await api.pressComment(id, kind))
      setError(null)
      refreshInfo()
      await reload()
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

  // Uma acção de mercado do menu: mostra a aba Transferência com o resultado
  const marketAction = async (f: () => Promise<TransferView>) => {
    try {
      setMarket(await f())
      setError(null)
      setTab('transfer')
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

  return (
    <div className="relative flex h-full flex-col">
      <ScreenHeader
        emblem={club ? { text: initials(club.shortName), fg: colours.fg, bg: colours.bg } : undefined}
        eyebrow={clubTag}
        title={title}
        right={
          game.mode === 'game' && person?.player && (
        <PurpleMenu
          label="Acção"
          align="right"
          buttonClass="cm-purple flex h-8 items-center gap-1.5 px-4"
          trigger={
            <>
              Acção <ChevronDown size={14} />
            </>
          }
          options={[...actions, common].filter((g) => g.length > 0).flatMap((g, i) => (i ? ['separator' as const, ...g] : g))}
          onChange={(k) => {
            if (k === 'contract' || k === 'renew') nav.go({ kind: 'contract', id })
            else if (k === 'offer' || k === 'loan' || k === 'price') setDeal(k)
            else if (k === 'option') void marketAction(() => api.exerciseOption(id))
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
            else if (k === 'praise' || k === 'criticise') void comment(k)
            else if (k === 'promote' || k === 'demote') void moveSquad(k === 'demote')
            else setReport(k as 'medical' | 'fitness')
          }}
        />
          )
        }
      />
      <Tabs
        tabs={TABS.map((t) =>
          t.key === 'injuries'
            ? { ...t, disabled: !person?.status }
            : t.key === 'transfer'
              ? { ...t, disabled: game.mode !== 'game' || !person?.player }
              : t,
        )}
        active={tab}
        onChange={setTab}
      />
      <div className="mx-2.5 mt-2.5 flex min-h-0 flex-1 flex-col">
        <ErrorBox className="mb-2.5">{error}</ErrorBox>
        {notice && <div className="mb-2.5 rounded bg-[#0f3a2a] px-2 py-1 text-center text-cm-yellow">{notice}</div>}
        {person && tab === 'profile' && <Profile p={person} />}
        {person && tab === 'contract' && <Contract p={person} />}
        {person && tab === 'injuries' && <Injuries p={person} />}
        {person && tab === 'history' && <History p={person} />}
        {person && tab === 'transfer' && <TransferTab personId={id} version={version} onChanged={() => void reload()} />}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
      {deal && market && person && (
        <TransferActions
          action={deal}
          view={market}
          name={person.name}
          onClose={() => setDeal(null)}
          onDone={(v) => {
            setDeal(null)
            setMarket(v)
            setTab('transfer')
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
              void api.releasePlayer(id).then(
                (t) => {
                  setNotice(t)
                  refreshInfo()
                  void reload()
                },
                (e) => setError(String(e)),
              )
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
          onDone={(text) => {
            setDiscipline(false)
            setNotice(text)
            refreshInfo()
            void reload()
          }}
        />
      )}
      {report && person && (
        <ReportDialog
          kind={report}
          personId={id}
          name={person.name}
          onClose={() => setReport(null)}
          onDone={(text) => {
            setReport(null)
            setNotice(text)
            // As mensagens novas contam no "por ler"
            void api.gameInfo().then((info) => info && game.update(info))
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
