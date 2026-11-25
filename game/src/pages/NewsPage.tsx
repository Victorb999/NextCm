import { ArrowLeftRight, BookOpen, Briefcase, FileSignature, Handshake, HeartPulse, Inbox, Mail, Newspaper, Trophy } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { api } from '../api'
import type { NewsCard } from '../bindings/NewsCard'
import type { NewsLink } from '../bindings/NewsLink'
import LinkedText from '../components/LinkedText'
import NewsCardBox from '../components/NewsCardBox'
import NewsSubsDialog from '../components/NewsSubsDialog'
import type { FriendlyInvite } from '../bindings/FriendlyInvite'
import type { NewsItem } from '../bindings/NewsItem'
import type { OfferView } from '../bindings/OfferView'
import { BottomBar, Empty, Panel, ScreenHeader, Tabs } from '../components/Screen'
import { OfferButtons } from '../components/TransferTab'
import { partLabel, ymd } from '../format'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'
import { setPendingAct } from '../personActions'

const TABS = [
  { key: 'all', label: 'Todas', icon: Inbox },
  { key: 'message', label: 'Mensagens', icon: Mail },
  { key: 'competition', label: 'Competições', icon: Trophy },
  { key: 'injury', label: 'Lesões e Suspensões', icon: HeartPulse },
]

// Os assuntos das abas de baixo (os "kind" das notícias no core)
const TOPICS: Record<string, string[]> = {
  press: ['contract', 'press'],
  transfer: ['transfer', 'deal'],
  jobs: ['jobs'],
}
const NOT_MESSAGES = ['competition', 'injury', 'transfer']

/** A notícia aparece na aba? */
function inTab(n: NewsItem, tab: string) {
  if (tab === 'all') return true
  if (tab === 'message') return !NOT_MESSAGES.includes(n.kind)
  return TOPICS[tab]?.includes(n.kind) ?? n.kind === tab
}

const WEEKDAYS = ['Dom', 'Seg', 'Ter', 'Qua', 'Qui', 'Sex', 'Sáb']
const MONTHS = ['Jan', 'Fev', 'Mar', 'Abr', 'Mai', 'Jun', 'Jul', 'Ago', 'Set', 'Out', 'Nov', 'Dez']

// A etiqueta de cada assunto na lista
type Label = [string, string, LucideIcon]
const KIND_LABEL: Record<string, Label> & { message: Label } = {
  message: ['Mensagem', 'bg-cm-hover text-cm-muted', Mail],
  competition: ['Competição', 'bg-[#0f2a3a] text-[#5bc0f0]', Trophy],
  injury: ['Lesões', 'bg-cm-red/20 text-cm-red', HeartPulse],
  transfer: ['Transferências', 'bg-cm-headdeep text-cm-purplelight', ArrowLeftRight],
  deal: ['Negócio', 'bg-cm-orange/20 text-cm-orange', Handshake],
  contract: ['Contrato', 'bg-cm-orange/20 text-cm-orange', FileSignature],
  press: ['Imprensa', 'bg-cm-hover text-cm-muted', Newspaper],
  jobs: ['Empregos', 'bg-cm-dlgdeep text-cm-mint', Briefcase],
}

/** "2025-07-01" -> "Ter 1 Jul", como na lista de notícias do CM. */
function newsDate(iso: string, part: number) {
  const [y, m, d] = ymd(iso)
  const wd = new Date(y, m - 1, d).getDay()
  return `${WEEKDAYS[wd]} ${d} ${MONTHS[m - 1]} ${partLabel(part)}`
}

/** O tamanho de 1rem em px (as linhas da lista têm 2rem, h-8). */
const remPx = () => parseFloat(getComputedStyle(document.documentElement).fontSize) || 16

/** As notícias de uma aba, da mais recente (em cima) para a mais antiga, como o core as guarda. */
const inOrder = (news: NewsItem[], tab: string) => news.filter((n) => inTab(n, tab))

/** Onde a aba abre: na mais antiga por ler (lê-se pela ordem em que chegaram), senão na
 * mais recente (ou nesta, com `latest`). */
function startAt(news: NewsItem[], tab: string, latest = false): number | null {
  const list = inOrder(news, tab)
  return ((latest ? undefined : list.findLast((n) => !n.read)) ?? list[0])?.id ?? null
}

const INVITE_STATUS: Record<string, string> = {
  accepted: 'Convite aceito: o amistoso está no Calendário.',
  declined: 'Convite recusado.',
  expired: 'O prazo para responder acabou.',
}

/** O convite de outro clube para um amigável: Aceitar / Recusar enquanto espera resposta. */
function InviteReply({ id }: { id: number }) {
  const [invite, setInvite] = useState<FriendlyInvite | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    api.friendlyInvite(id).then(setInvite, () => setInvite(null))
  }, [id])
  if (!invite) return null
  const answer = (accept: boolean) =>
    api.answerFriendlyInvite(id, accept).then(
      (i) => {
        setInvite(i)
        setError(null)
      },
      (e) => setError(String(e)),
    )
  return (
    <div className="mt-4 flex items-center gap-1 border-t border-cm-line pt-3">
      {invite.status === 'pending' ? (
        <>
          <button className="cm-purple h-7 w-36 text-cm-yellow" onClick={() => void answer(true)}>
            Aceitar
          </button>
          <button className="cm-purple h-7 w-36" onClick={() => void answer(false)}>
            Recusar
          </button>
        </>
      ) : (
        <span className="text-[15px] text-cm-mint">{INVITE_STATUS[invite.status] ?? ''}</span>
      )}
      {error && <span className="ml-2 text-[14px] text-cm-red">{error}</span>}
    </div>
  )
}

/** "Notícias para <treinador>": lista em cima, texto da notícia em baixo. */
export default function NewsPage({ latest, id }: { latest?: boolean; id?: number }) {
  const game = useGame()
  const nav = useNav()
  useEscBack()
  const [news, setNews] = useState<NewsItem[]>([])
  const [tab, setTab] = useState('all')
  // Ao voltar (de um nome clicado numa notícia), abre na que estava aberta
  const [selected, setSelected] = useState<number | null>(id ?? null)
  const [subsOpen, setSubsOpen] = useState(false)
  // Onde os botões da proposta da mensagem aparecem (na barra da lista)

  // Recarrega quando o dia avança ou uma notícia é lida; ao abrir, fixa a primeira por ler
  // (escolhida uma vez: ler não a faz saltar para a seguinte)
  useEffect(() => {
    api.listNews().then(
      (list) => {
        setNews(list)
        setSelected((s) => s ?? startAt(list, tab, latest))
      },
      () => {},
    )
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [game.info?.date, game.info?.unreadNews])
  const changeTab = (t: string) => {
    setTab(t)
    setSelected(startAt(news, t))
  }

  // A mais recente em cima
  const shown = useMemo(() => inOrder(news, tab), [news, tab])
  const current = shown.find((n) => n.id === selected) ?? shown[0]
  // A lista só desenha as linhas à vista (um save longo tem mais de mil notícias)
  const listRef = useRef<HTMLDivElement>(null)
  const [view, setView] = useState({ top: 0, height: 400 })
  const row = 2 * remPx()
  const first = Math.max(0, Math.floor(view.top / row) - 8) & ~1 // par: a zebra não troca
  const last = Math.min(shown.length, Math.ceil((view.top + view.height) / row) + 8)
  // A altura no máximo a da janela (a meio do layout a lista ainda pode estar sem limite)
  const onScroll = () => {
    const el = listRef.current
    if (el) setView({ top: el.scrollTop, height: Math.min(el.clientHeight, window.innerHeight) })
  }
  useEffect(() => {
    const el = listRef.current
    if (!el) return
    const watch = new ResizeObserver(() => setView({ top: el.scrollTop, height: Math.min(el.clientHeight, window.innerHeight) }))
    watch.observe(el)
    return () => watch.disconnect()
  }, [])
  const currentIndex = shown.findIndex((n) => n.id === current?.id)
  useEffect(() => {
    const el = listRef.current
    if (!el || currentIndex < 0) return
    const top = currentIndex * row
    if (top < el.scrollTop) el.scrollTop = top
    else if (top + row > el.scrollTop + el.clientHeight) el.scrollTop = top + row - el.clientHeight
    setView({ top: el.scrollTop, height: Math.min(el.clientHeight, window.innerHeight) })
  }, [currentIndex, row])

  // Abrir uma notícia marca-a como lida
  useEffect(() => {
    if (current && !current.read) api.readNews(current.id).then(game.update, () => {})
  }, [current, game.update])

  // A notícia aberta fica no histórico: o Atrás volta a ela, e não à próxima por ler
  const { replace } = nav
  const openId = current?.id
  useEffect(() => {
    if (openId !== undefined) replace({ kind: 'news', latest, id: openId })
  }, [openId, latest, replace])

  // A proposta da mensagem (botões para responder) e o diálogo do contrato
  const [loaded, setOffer] = useState<OfferView | null>(null)
  const offerId = current?.offerId ?? null
  const offer = loaded && loaded.id === offerId ? loaded : null
  useEffect(() => {
    if (offerId !== null) api.offerView(offerId).then(setOffer, () => setOffer(null))
  }, [offerId, game.info?.date])

  // Os nomes de jogadores e clubes da notícia, como ligações
  const [links, setLinks] = useState<{ id: number; list: NewsLink[] }>({ id: -1, list: [] })
  const currentId = current?.id ?? null
  useEffect(() => {
    if (currentId !== null)
      api.newsLinks(currentId).then(
        (list) => setLinks({ id: currentId, list }),
        () => {},
      )
  }, [currentId])
  const currentLinks = links.id === currentId ? links.list : []
  // O quadro ao lado do texto (a pessoa ou o clube de que fala)
  const [card, setCard] = useState<{ id: number; card: NewsCard | null }>({ id: -1, card: null })
  useEffect(() => {
    if (currentId !== null)
      api.newsCard(currentId).then(
        (c) => setCard({ id: currentId, card: c }),
        () => setCard({ id: currentId, card: null }),
      )
  }, [currentId])
  const currentCard = card.id === currentId ? card.card : null

  const readNext = () => {
    // A seguinte por ler, pela ordem em que chegaram (de baixo para cima)
    const next = shown.findLast((n) => !n.read && n.id !== current?.id)
    if (next) setSelected(next.id)
  }

  const unread = news.filter((n) => !n.read).length
  const count = (t: string) => news.filter((n) => inTab(n, t)).length
  return (
    <div className="relative flex h-full flex-col">
      {subsOpen && <NewsSubsDialog onClose={() => setSubsOpen(false)} />}
      <ScreenHeader
        icon={<Newspaper size={22} />}
        eyebrow={game.info?.managerName}
        title="Notícias"
        right={
          <span className="rounded-md bg-black/30 px-3 py-1.5 font-mono text-[14px] uppercase tracking-wider text-cm-dim">
            Não lidas: <b className={unread ? 'text-cm-yellow' : 'text-white'}>{unread}</b>
          </span>
        }
      />
      <Tabs tabs={TABS.map((t) => ({ ...t, label: `${t.label} (${count(t.key)})` }))} active={tab} onChange={changeTab} />
      <div className="mx-2 mt-2 flex min-h-0 flex-1 flex-col gap-2">
        <Panel
          title="Caixa de Mensagens"
          right={
            <span className="flex items-center gap-1.5">
              <button className="cm-purple h-6 w-32" onClick={() => setSubsOpen(true)}>
                Assinaturas
              </button>
              <button className="cm-purple h-6 w-32" disabled={!shown.some((n) => !n.read && n.id !== current?.id)} onClick={readNext}>
                Ler Próxima
              </button>
            </span>
          }
          className="h-[205px] shrink-0"
        >
          <div ref={listRef} onScroll={onScroll} className="min-h-0 flex-1 overflow-auto">
            {shown.length === 0 && <Empty className="p-4">Sem notícias.</Empty>}
            {/* O espaço das linhas por desenhar fica no padding de dentro (o de fora não encolhe) */}
            <div className="cm-zebra" style={{ paddingTop: first * row, paddingBottom: (shown.length - last) * row }}>
              {shown.slice(first, last).map((n) => {
                const [label, colour, Icon] = KIND_LABEL[n.kind] ?? KIND_LABEL.message
                return (
                  <div
                    key={n.id}
                    onMouseDown={() => setSelected(n.id)}
                    data-current={n.id === current?.id || undefined}
                    className={`flex h-8 cursor-pointer items-center gap-3 px-3 text-[16px]  ${n.id === current?.id ? '!bg-cm-sel' : ''}`}
                  >
                    <span className="cm-cell w-32 shrink-0 text-center font-mono text-[14px] text-white">{newsDate(n.date, n.part)}</span>
                    <span className={`min-w-0 flex-1 truncate ${n.read ? 'text-white' : 'font-bold text-cm-yellow'}`}>{n.title}</span>
                    {n.awaiting && <span className="shrink-0 text-[13px] font-bold text-cm-magenta">Responder</span>}
                    <span
                      className={`flex w-36 shrink-0 items-center justify-center gap-1 rounded px-1.5 font-mono text-[12px] font-bold uppercase tracking-wider ${colour}`}
                    >
                      <Icon size={12} className="shrink-0" />
                      {label}
                    </span>
                  </div>
                )
              })}
            </div>
          </div>
        </Panel>
        <Panel title={current ? newsDate(current.date, current.part) : 'Mensagem'} className="min-h-0 flex-1">
          <div className="min-h-0 flex-1 overflow-auto px-5 py-3">
            {current && (
              <>
                <div className="pb-3 font-title text-[22px] font-bold text-cm-yellow">{current.title}</div>
                <div className="flex items-start gap-4">
                  <div className="min-w-0 flex-1 whitespace-pre-line text-[18px] leading-relaxed text-cm-text">
                    <LinkedText text={current.body} links={currentLinks} />
                  </div>
                  {currentCard && <NewsCardBox card={currentCard} />}
                </div>
                {current.link?.startsWith('press:') && (
                  <div className="mt-4 flex gap-1 border-t border-cm-line pt-3">
                    <button className="cm-purple h-7 w-48" onClick={() => nav.go({ kind: 'press', id: Number(current.link!.slice(6)) })}>
                      Ir à Entrevista
                    </button>
                    {current.awaiting && (
                      <button
                        className="cm-purple h-7 w-48"
                        onClick={() =>
                          api.skipPress(Number(current.link!.slice(6))).then(
                            () => {
                              api.listNews().then(setNews, () => {})
                              api.gameInfo().then(
                                (info) => info && game.update(info),
                                () => {},
                              )
                            },
                            () => {},
                          )
                        }
                      >
                        Mandar o Auxiliar
                      </button>
                    )}
                  </div>
                )}
                {current.link?.startsWith('sheet:') && (
                  <div className="mt-4 flex gap-1 border-t border-cm-line pt-3">
                    <button className="cm-purple h-7 w-48" onClick={() => nav.go({ kind: 'match', id: Number(current.link!.slice(6)) })}>
                      Ver o Jogo
                    </button>
                  </div>
                )}
                {current.link?.startsWith('scout:') && (
                  <div className="mt-4 flex gap-1 border-t border-cm-line pt-3">
                    <button
                      className="cm-purple h-7 w-48"
                      onClick={() => nav.go({ kind: 'person', id: Number(current.link!.slice(6)), tab: 'scout' })}
                    >
                      Ver Relatório
                    </button>
                  </div>
                )}
                {current.link?.startsWith('friendly:') && <InviteReply key={current.id} id={Number(current.link.slice(9))} />}
                {current.link === 'squadNumbers' && (
                  <div className="mt-4 border-t border-cm-line pt-3">
                    <button
                      className="cm-purple h-7 w-48"
                      disabled={!game.info?.numbersOpen}
                      title={game.info?.numbersOpen ? undefined : 'Os números só podem ser mudados na pré-temporada'}
                      onClick={() => nav.go({ kind: 'squadNumbers' })}
                    >
                      Definir Números
                    </button>
                  </div>
                )}
                {offer && (
                  <div className="mt-4 border-t border-cm-line pt-3">
                    <OfferButtons
                      offer={offer}
                      onChange={(o) => {
                        setOffer(o)
                        api.listNews().then(setNews, () => {})
                      }}
                      onNegotiate={() => {
                        // O staff negoceia no diálogo do perfil
                        if (offer.kind === 'staff') {
                          setPendingAct(offer.personId, 'staffContract')
                          nav.go({ kind: 'person', id: offer.personId, tab: 'contract' })
                        } else nav.go({ kind: 'contract', id: offer.personId })
                      }}
                    />
                    {offer.kind !== 'job' && (
                      <button className="cm-purple mt-1 h-7 min-w-36" onClick={() => nav.go({ kind: 'person', id: offer.personId })}>
                        Ver {offer.name}
                      </button>
                    )}
                  </div>
                )}
              </>
            )}
          </div>
        </Panel>
      </div>
      <div className="mt-2">
        <Tabs
          tabs={[
            { key: 'press', label: 'Contratos e Imprensa', icon: FileSignature },
            { key: 'transfer', label: 'Transferências', icon: ArrowLeftRight },
            { key: 'jobs', label: 'Empregos', icon: Briefcase },
            { key: 'records', label: 'Registros', icon: BookOpen, disabled: true },
          ]}
          active={tab in TOPICS ? tab : ''}
          onChange={changeTab}
        />
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
