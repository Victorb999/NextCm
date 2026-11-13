import { useLoad } from '../useLoad'
import Contract from './person/Contract'
import Injuries from './person/Injuries'
import PersonalPanel from './person/PersonalPanel'
import PersonHeader from './person/PersonHeader'
import Profile from './person/Profile'
import { ChevronDown } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import type { PersonProfile } from '../bindings/PersonProfile'
import { DialogFrame, Overlay } from '../components/Dialog'
import DisciplineDialog from '../components/DisciplineDialog'
import PersonRecords from '../components/PersonRecords'
import PlayerHistory from '../components/PlayerHistory'
import ReportDialog from '../components/ReportDialog'
import SquadRoleDialog from '../components/SquadRoleDialog'
import { BottomBar, ErrorBox, Panel, PurpleMenu, Tabs } from '../components/Screen'
import type { TransferView } from '../bindings/TransferView'
import TransferActions, { type TransferAction } from '../components/TransferActions'
import TextDialog from '../components/TextDialog'
import TransferTab from '../components/TransferTab'
import { compareFirst, setCompareFirst } from '../compare'
import { personActions, takePendingAct } from '../personActions'
import { useGame } from '../game'
import { useNav } from '../nav'

const TABS = [
  { key: 'profile', label: 'Perfil Geral' },
  { key: 'contract', label: 'Contrato & Transferência' },
  { key: 'history', label: 'Historial / Carreira' },
  { key: 'injuries', label: 'Lesões & Castigos' },
  { key: 'records', label: 'Marcos & Recordes' },
]

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
  useLoad(() => api.getPerson(id), [id], setPerson, setError)
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
  const marketButtons = pick([
    'list',
    'unlist',
    'price',
    'loanlist',
    'unloanlist',
    'shop',
    'offer',
    'loan',
    'option',
    'shortlist',
    'unshortlist',
    'scout',
  ])
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
    } else if (k === 'list' || k === 'unlist' || k === 'loanlist' || k === 'unloanlist')
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
            tabs={(person?.player && game.mode === 'game' ? TABS : TABS.filter((t) => t.key !== 'injuries' && t.key !== 'records')).map(
              (t) =>
                person?.ghost && t.key !== 'profile'
                  ? { ...t, disabled: true }
                  : t.key === 'injuries'
                    ? {
                        ...t,
                        label: person?.status?.injury || person?.status?.suspended ? `${t.label} •` : t.label,
                        disabled: !person?.status,
                      }
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
