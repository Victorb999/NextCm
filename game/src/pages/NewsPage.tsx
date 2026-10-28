import { useEffect, useMemo, useState } from 'react'
import { api } from '../api'
import type { NewsLink } from '../bindings/NewsLink'
import LinkedText from '../components/LinkedText'
import type { NewsItem } from '../bindings/NewsItem'
import type { OfferView } from '../bindings/OfferView'
import { BottomBar, Tabs, TitleBand } from '../components/Screen'
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

/** "2025-07-01" -> "Ter 1 Jul", como na lista de notícias do CM. */
function newsDate(iso: string) {
  const [y, m, d] = iso.split('-').map(Number)
  const wd = new Date(y, m - 1, d).getDay()
  return `${WEEKDAYS[wd]} ${d} ${MONTHS[m - 1]}`
}

/** "Notícias para <treinador>": lista em cima, texto da notícia em baixo. */
export default function NewsPage() {
  const game = useGame()
  const nav = useNav()
  useEscBack()
  const [news, setNews] = useState<NewsItem[]>([])
  const [tab, setTab] = useState('all')
  const [selected, setSelected] = useState<number | null>(null)

  // Recarrega quando o dia avança ou uma notícia é lida
  useEffect(() => {
    api.listNews().then(setNews, () => {})
  }, [game.info?.date, game.info?.unreadNews])

  const shown = useMemo(() => news.filter((n) => inTab(n, tab)), [news, tab])
  const current = shown.find((n) => n.id === selected) ?? shown[0]

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

  return (
    <div className="flex h-full flex-col">
      <TitleBand title={game.info ? `Notícias para ${game.info.managerName}` : 'Notícias'} fg="#000000" bg="#f4f4f4" />
      <Tabs tabs={TABS} active={tab} onChange={setTab} />
      <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 flex-col">
        <div className="h-[118px] shrink-0 overflow-auto border border-cm-line bg-black/60">
          {shown.length === 0 && <div className="p-4 text-center text-cm-magenta">Sem notícias.</div>}
          {shown.map((n) => (
            <div
              key={n.id}
              onMouseDown={() => setSelected(n.id)}
              className={`flex h-[21px] items-center gap-3 px-2 text-[14px] ${n.id === current?.id ? 'bg-cm-sel' : ''}`}
            >
              <span className="w-24 shrink-0 bg-[#202020] px-1 text-white">{newsDate(n.date)}</span>
              <span className={`truncate ${n.read ? 'text-white' : 'font-bold text-cm-yellow'}`}>{n.title}</span>
              {n.awaiting && <span className="ml-auto shrink-0 pr-1 text-[13px] font-bold text-cm-magenta">Responder</span>}
            </div>
          ))}
        </div>
        <div className="flex h-8 shrink-0 items-center justify-end gap-2 py-1">
          <button className="cm-purple h-6 w-40" disabled={!shown.some((n) => !n.read && n.id !== current?.id)} onClick={readNext}>
            Ler Próxima
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-auto border border-cm-line bg-cm-panel/70 px-5 py-3">
          {current && (
            <>
              <div className="pb-3 font-title text-[22px] font-bold text-cm-yellow">{current.title}</div>
              <div className="whitespace-pre-line font-title text-[19px] leading-snug text-white">
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
      </div>
      <div className="mt-1.5">
        <Tabs
          tabs={[
            { key: 'press', label: 'Contratos e Imprensa' },
            { key: 'transfer', label: 'Transferências' },
            { key: 'jobs', label: 'Empregos' },
            { key: 'records', label: 'Registos', disabled: true },
          ]}
          active={tab in TOPICS ? tab : ''}
          onChange={setTab}
        />
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
