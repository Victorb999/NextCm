import { useEffect, useMemo, useState } from 'react'
import { api } from '../api'
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

  const shown = useMemo(() => news.filter((n) => tab === 'all' || n.kind === tab), [news, tab])
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
              <div className="whitespace-pre-line font-title text-[19px] leading-snug text-white">{current.body}</div>
              {offer && (
                <div className="mt-4 border-t border-cm-line pt-3">
                  <OfferButtons
                    offer={offer}
                    onChange={setOffer}
                    onNegotiate={() => nav.go({ kind: 'contract', id: offer.personId })}
                  />
                </div>
              )}
            </>
          )}
        </div>
      </div>
      <div className="mt-1.5">
        <Tabs
          tabs={[
            { key: 'press', label: 'Contratos e Imprensa', disabled: true },
            { key: 'transfer', label: 'Transferências' },
            { key: 'jobs', label: 'Empregos', disabled: true },
            { key: 'records', label: 'Registos', disabled: true },
          ]}
          active={tab === 'transfer' ? 'transfer' : ''}
          onChange={(k) => {
            if (k === 'transfer') setTab('transfer')
          }}
        />
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
