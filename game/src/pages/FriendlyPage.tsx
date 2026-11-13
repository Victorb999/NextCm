import { useEffect, useMemo, useState } from 'react'
import { api } from '../api'
import type { FriendlyClub } from '../bindings/FriendlyClub'
import type { FriendlyDay } from '../bindings/FriendlyDay'
import type { TourNation } from '../bindings/TourNation'
import { BottomBar, Empty, ErrorBox, PanelTitle, PurpleMenu, ScreenHeader } from '../components/Screen'
import { shortDate } from '../format'
import { setFriendlyPlan } from '../friendlyPlan'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'

// Marcar amigáveis, como no CM: Excursão (Escolher País → Escolher Datas → Escolher Clube, um
// por data) ou Amistoso em Casa / Fora (Escolher Datas e depois "Convidar Clube" na página do
// clube). As datas livres e as regras vêm do core.

type Mode = 'home' | 'away' | 'tour'
type Step = 'nation' | 'dates' | 'clubs'

const PAGE = 17
const CONTINENTS = ['AFR', 'ASI', 'EUR', 'NAM', 'OCE', 'SAM'] as const
const TITLE: Record<Mode, string> = { home: 'Amistoso em Casa', away: 'Amistoso Fora', tour: 'Combinar Excursão Amistosa' }

export default function FriendlyPage({ mode }: { mode: Mode }) {
  const nav = useNav()
  const game = useGame()
  useEscBack()
  const [step, setStep] = useState<Step>(mode === 'tour' ? 'nation' : 'dates')
  const [error, setError] = useState<string | null>(null)
  // A excursão: o país, as datas e o clube de cada data
  const [nations, setNations] = useState<TourNation[]>([])
  const [continent, setContinent] = useState('')
  const [nation, setNation] = useState<TourNation | null>(null)
  const [days, setDays] = useState<FriendlyDay[]>([])
  const [chosen, setChosen] = useState<string[]>([])
  const [page, setPage] = useState(0)
  const [picks, setPicks] = useState<number[]>([])
  const [found, setFound] = useState<{ date: string; list: FriendlyClub[] } | null>(null)

  useEffect(() => {
    if (mode === 'tour') api.tourNations().then(setNations, (e) => setError(String(e)))
  }, [mode])
  useEffect(() => {
    api.friendlyDates(chosen).then(setDays, (e) => setError(String(e)))
  }, [chosen])
  // O clube da data em que se está a escolher
  const pickDate = chosen[picks.length]
  useEffect(() => {
    if (step !== 'clubs' || !nation || !pickDate) return
    api.friendlyClubs(nation.id, [pickDate]).then(
      (list) => setFound({ date: pickDate, list }),
      (e) => setError(String(e)),
    )
  }, [step, nation, pickDate])
  // Os clubes desta data (os de outra data ainda são da escolha anterior)
  const clubs = found && found.date === pickDate ? found.list : null

  const shownNations = useMemo(() => nations.filter((n) => !continent || n.continent === continent), [nations, continent])
  const pages = Math.max(1, Math.ceil(days.length / PAGE))
  const pageDays = days.slice(page * PAGE, page * PAGE + PAGE)

  const toggle = (d: FriendlyDay) => {
    if (d.status === 'chosen') setChosen(chosen.filter((c) => c !== d.date))
    else if (d.status === 'free') setChosen(mode === 'tour' ? [...chosen, d.date].sort() : [d.date])
  }

  const finish = async (games: number[]) => {
    try {
      await api.arrangeFriendlies(
        'tour',
        chosen.map((date, i) => ({ date, club: games[i] ?? 0 })),
      )
      if (game.info) nav.go({ kind: 'club', id: game.info.club.id, tab: 'fixtures' })
    } catch (e) {
      setError(String(e))
      setPicks([])
    }
  }
  const pickClub = (club: number) => {
    const next = [...picks, club]
    if (next.length < chosen.length) setPicks(next)
    else void finish(next)
  }

  const next = () => {
    setError(null)
    if (step === 'nation') setStep('dates')
    else if (step === 'dates' && mode === 'tour') {
      setPicks([])
      setStep('clubs')
    } else if (step === 'dates' && (mode === 'home' || mode === 'away') && chosen[0]) {
      // O convite: o treinador escolhe o clube na página dele
      setFriendlyPlan({ kind: mode, date: chosen[0] })
      nav.go({ kind: 'nations' })
    }
  }
  const back = () => {
    setError(null)
    if (step === 'clubs') setStep('dates')
    else if (step === 'dates' && mode === 'tour') setStep('nation')
    else nav.back()
  }
  const canNext = step === 'nation' ? nation !== null : step === 'dates' ? chosen.length > 0 : false

  const title = mode === 'tour' && step !== 'nation' && nation ? `Programar Excursão ${nation.name}` : TITLE[mode]
  const subtitle =
    step === 'nation'
      ? 'Escolher País'
      : step === 'dates'
        ? 'Escolher Datas'
        : `Escolher Clube · ${pickDate ? shortDate(pickDate) : ''} (${picks.length + 1} de ${chosen.length})`

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader club={game.info?.club} eyebrow={game.info?.club.name} title={title} />
      <div className="mx-2 flex min-h-0 flex-1 flex-col gap-2">
        <ErrorBox>{error}</ErrorBox>
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-md bg-cm-surface">
          <PanelTitle
            title={subtitle}
            right={
              step === 'nation' ? (
                <PurpleMenu
                  label="Filtrar"
                  align="right"
                  options={[{ key: '', label: 'Todos' }, ...CONTINENTS.map((c) => ({ key: c, label: c }))]}
                  value={continent}
                  onChange={setContinent}
                />
              ) : step === 'dates' ? (
                <>
                  <button className="cm-purple h-6 w-8" disabled={page === 0} onClick={() => setPage(0)}>
                    «
                  </button>
                  <button className="cm-purple h-6 w-8" disabled={page === 0} onClick={() => setPage(page - 1)}>
                    ‹
                  </button>
                  <span className="w-20 text-center font-mono text-[12px] text-cm-yellow">
                    Pg {page + 1}/{pages}
                  </span>
                  <button className="cm-purple h-6 w-8" disabled={page >= pages - 1} onClick={() => setPage(page + 1)}>
                    ›
                  </button>
                  <button className="cm-purple h-6 w-8" disabled={page >= pages - 1} onClick={() => setPage(pages - 1)}>
                    »
                  </button>
                </>
              ) : null
            }
          />
          {step === 'nation' && (
            <div className="cm-zebra grid min-h-0 flex-1 auto-rows-[26px] grid-cols-2 content-start overflow-auto">
              {shownNations.map((n) => (
                <button
                  key={n.id}
                  className={`flex items-center gap-2 px-3 text-left ${n.id === nation?.id ? '!bg-cm-sel' : 'hover:bg-cm-hover'}`}
                  onClick={() => setNation(n)}
                  onDoubleClick={() => {
                    setNation(n)
                    setStep('dates')
                  }}
                >
                  <span className="min-w-0 flex-1 truncate font-title text-[16px] text-white">{n.name}</span>
                  <span className="w-10 text-[12px] font-bold text-cm-yellow">{n.continent}</span>
                </button>
              ))}
            </div>
          )}
          {step === 'dates' && (
            <div className="cm-zebra min-h-0 flex-1 overflow-auto">
              {pageDays.map((d) => {
                const f = d.fixture
                const home = f?.home.id === game.info?.club.id
                const opponent = f ? (home ? f.away : f.home) : null
                return (
                  <div
                    key={d.date}
                    className={`flex h-[26px] items-center gap-3 px-3 ${d.status === 'free' || d.status === 'chosen' ? 'cursor-pointer hover:bg-cm-hover' : ''}`}
                    onClick={() => toggle(d)}
                  >
                    <span className="cm-cell w-28 shrink-0 text-center text-[13px] text-white">{shortDate(d.date)}</span>
                    {f && opponent ? (
                      <>
                        <span className="min-w-0 flex-1 truncate font-title text-[16px] text-white">{opponent.shortName}</span>
                        <span className="w-6 shrink-0 text-center font-bold text-cm-magenta">{home ? 'C' : 'F'}</span>
                        <span className="w-52 shrink-0 truncate text-cm-orange">{f.competition}</span>
                      </>
                    ) : (
                      <span
                        className={`flex-1 font-title text-[16px] ${
                          d.status === 'chosen' ? 'text-cm-yellow' : d.status === 'free' ? 'text-white' : 'text-cm-purplelight'
                        }`}
                      >
                        {d.status === 'chosen' ? 'Data Escolhida' : d.status === 'free' ? 'Data Disponível' : 'Data Indisponível'}
                      </span>
                    )}
                  </div>
                )
              })}
            </div>
          )}
          {step === 'clubs' &&
            (clubs === null ? (
              <Empty>A procurar clubes…</Empty>
            ) : clubs.length === 0 ? (
              <Empty>Nenhum clube deste país pode jogar nesta data.</Empty>
            ) : (
              <div className="cm-zebra grid min-h-0 flex-1 auto-rows-[26px] grid-cols-2 content-start overflow-auto">
                {clubs.map((c) => (
                  <button
                    key={c.club.id}
                    className="flex items-center gap-2 px-3 text-left hover:bg-cm-hover"
                    onClick={() => pickClub(c.club.id)}
                  >
                    <span className="min-w-0 flex-1 truncate font-title text-[16px] text-white">{c.club.shortName}</span>
                    <span className="w-14 truncate text-[12px] font-bold text-cm-orange">{c.division}</span>
                  </button>
                ))}
              </div>
            ))}
        </div>
      </div>
      <BottomBar onLeft={back} onRight={canNext ? next : undefined} />
    </div>
  )
}
