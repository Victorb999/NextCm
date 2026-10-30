import { Newspaper } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { api } from '../api'
import type { NewsLink } from '../bindings/NewsLink'
import LinkedText from '../components/LinkedText'
import type { NewsItem } from '../bindings/NewsItem'
import type { OfferView } from '../bindings/OfferView'
import { BottomBar, Empty, Panel, ScreenHeader, Tabs } from '../components/Screen'
import { OfferButtons } from '../components/TransferTab'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'

const TABS = [
  { key: 'all', label: 'Todas' },
  { key: 'message', label: 'Mensagens' },
  { key: 'competition', label: 'Competições' },
  { key: 'injury', label: 'Lesões e Suspensões' },
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
const KIND_LABEL: Record<string, [string, string]> = {
  message: ['Mensagem', 'bg-[#2a2b32] text-[#c8c8d0]'],
  competition: ['Competição', 'bg-[#0f2a3a] text-[#5bc0f0]'],
  injury: ['Lesões', 'bg-[#3a1010] text-[#ff7070]'],
  transfer: ['Transferências', 'bg-[#2a1a45] text-[#b48cf0]'],
  deal: ['Negócio', 'bg-[#3a2508] text-cm-orange'],
  contract: ['Contrato', 'bg-[#3a2508] text-cm-orange'],
  press: ['Imprensa', 'bg-[#2a2b32] text-[#c8c8d0]'],
  jobs: ['Empregos', 'bg-[#0f3a2a] text-[#4edea3]'],
}

/** "2025-07-01" -> "Ter 1 Jul", como na lista de notícias do CM. */
function newsDate(iso: string) {
  const [y, m, d] = iso.split('-').map(Number)
  const wd = new Date(y, m - 1, d).getDay()
  return `${WEEKDAYS[wd]} ${d} ${MONTHS[m - 1]}`
}

/** As notícias de uma aba, da mais antiga para a mais recente (o core guarda-as ao contrário). */
const inOrder = (news: NewsItem[], tab: string) => news.filter((n) => inTab(n, tab)).reverse()

/** Onde a aba abre: na primeira por ler, senão na mais recente. */
function startAt(news: NewsItem[], tab: string): number | null {
  const list = inOrder(news, tab)
  return (list.find((n) => !n.read) ?? list[list.length - 1])?.id ?? null
}

/** "Notícias para <treinador>": lista em cima, texto da notícia em baixo. */
export default function NewsPage() {
  const game = useGame()
  const nav = useNav()
  useEscBack()
  const [news, setNews] = useState<NewsItem[]>([])
  const [tab, setTab] = useState('all')
  const [selected, setSelected] = useState<number | null>(null)

  // Recarrega quando o dia avança ou uma notícia é lida; ao abrir, fixa a primeira por ler
  // (escolhida uma vez: ler não a faz saltar para a seguinte)
  useEffect(() => {
    api.listNews().then((list) => {
      setNews(list)
      setSelected((s) => s ?? startAt(list, tab))
    }, () => {})
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [game.info?.date, game.info?.unreadNews])
  const changeTab = (t: string) => {
    setTab(t)
    setSelected(startAt(news, t))
  }

  // Da mais antiga para a mais recente (o core guarda-as ao contrário)
  const shown = useMemo(() => inOrder(news, tab), [news, tab])
  const current = shown.find((n) => n.id === selected) ?? shown[shown.length - 1]
  const listRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    listRef.current?.querySelector('[data-current]')?.scrollIntoView({ block: 'nearest' })
  }, [current?.id])

  // Abrir uma notícia marca-a como lida
  useEffect(() => {
    if (current && !current.read) api.readNews(current.id).then(game.update, () => {})
  }, [current, game.update])

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
    if (currentId !== null) api.newsLinks(currentId).then((list) => setLinks({ id: currentId, list }), () => {})
  }, [currentId])
  const currentLinks = links.id === currentId ? links.list : []

  const readNext = () => {
    const next = shown.find((n) => !n.read && n.id !== current?.id)
    if (next) setSelected(next.id)
  }

  const unread = news.filter((n) => !n.read).length
  const count = (t: string) => news.filter((n) => inTab(n, t)).length
  return (
    <div className="flex h-full flex-col">
      <ScreenHeader
        icon={<Newspaper size={22} />}
        eyebrow={game.info?.managerName}
        title="Notícias"
        right={
          <span className="rounded-md bg-black/30 px-3 py-1.5 font-mono text-[12px] uppercase tracking-wider text-[#86869a]">
            Por ler: <b className={unread ? 'text-cm-yellow' : 'text-white'}>{unread}</b>
          </span>
        }
      />
      <Tabs tabs={TABS.map((t) => ({ ...t, label: `${t.label} (${count(t.key)})` }))} active={tab} onChange={changeTab} />
      <div className="mx-2 mt-2 flex min-h-0 flex-1 flex-col gap-2">
        <Panel
          title="Caixa de Mensagens"
          right={
            <button className="cm-purple h-6 w-32" disabled={!shown.some((n) => !n.read && n.id !== current?.id)} onClick={readNext}>
              Ler Próxima
            </button>
          }
          className="h-[205px] shrink-0"
        >
          <div ref={listRef} className="cm-zebra min-h-0 flex-1 overflow-auto">
            {shown.length === 0 && <Empty className="p-4">Sem notícias.</Empty>}
            {shown.map((n) => {
              const [label, colour] = KIND_LABEL[n.kind] ?? KIND_LABEL.message
              return (
                <div
                  key={n.id}
                  onMouseDown={() => setSelected(n.id)}
                  data-current={n.id === current?.id || undefined}
                  className={`flex h-6 cursor-pointer items-center gap-3 px-3 text-[14px] ${n.id === current?.id ? '!bg-cm-sel' : ''}`}
                >
                  <span className="cm-cell w-24 shrink-0 text-center font-mono text-[12px] text-white">{newsDate(n.date)}</span>
                  <span className={`min-w-0 flex-1 truncate ${n.read ? 'text-white' : 'font-bold text-cm-yellow'}`}>{n.title}</span>
                  {n.awaiting && <span className="shrink-0 text-[13px] font-bold text-cm-magenta">Responder</span>}
                  <span className={`w-28 shrink-0 rounded px-1.5 text-center font-mono text-[10.5px] font-bold uppercase tracking-wider ${colour}`}>{label}</span>
                </div>
              )
            })}
          </div>
        </Panel>
        <Panel title={current ? newsDate(current.date) : 'Mensagem'} className="min-h-0 flex-1">
          <div className="min-h-0 flex-1 overflow-auto px-5 py-3">
            {current && (
              <>
                <div className="pb-3 font-title text-[22px] font-bold text-cm-yellow">{current.title}</div>
                <div className="whitespace-pre-line text-[16px] leading-relaxed text-[#e3e1e9]">
                  <LinkedText text={current.body} links={currentLinks} />
                </div>
                {current.link?.startsWith('press:') && (
                  <div className="mt-4 flex gap-1 border-t border-cm-line pt-3">
                    <button className="cm-purple h-7 w-48" onClick={() => nav.go({ kind: 'press', id: Number(current.link!.slice(6)) })}>
                      Ir à Conferência
                    </button>
                  </div>
                )}
                {current.link === 'squadNumbers' && (
                  <div className="mt-4 border-t border-cm-line pt-3">
                    <button
                      className="cm-purple h-7 w-48"
                      disabled={!game.info?.numbersOpen}
                      title={game.info?.numbersOpen ? undefined : 'Os números só se mudam na pré-época'}
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
                      onNegotiate={() => nav.go({ kind: 'contract', id: offer.personId })}
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
            { key: 'press', label: 'Contratos e Imprensa' },
            { key: 'transfer', label: 'Transferências' },
            { key: 'jobs', label: 'Empregos' },
            { key: 'records', label: 'Registos', disabled: true },
          ]}
          active={tab in TOPICS ? tab : ''}
          onChange={changeTab}
        />
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
