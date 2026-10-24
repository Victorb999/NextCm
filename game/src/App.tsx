import { ArrowLeft, ArrowRight } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { api, hasBackend } from './api'
import type { CompetitionListItem } from './bindings/CompetitionListItem'
import CascadeMenu, { type MenuNode } from './components/CascadeMenu'
import type { Fixture } from './bindings/Fixture'
import type { StandingRow } from './bindings/StandingRow'
import { DialogFrame, Overlay } from './components/Dialog'
import Loading from './components/Loading'
import MatchDay from './components/MatchDay'
import OptionsDialog from './components/OptionsDialog'
import { competitionsMenu } from './competitionsMenu'
import { GameProvider, useGame } from './game'
import { NavProvider, useNav } from './nav'
import { useSettings } from './settings'
import ClubHistoryPage from './pages/ClubHistoryPage'
import ClubPage from './pages/ClubPage'
import CompetitionPage from './pages/CompetitionPage'
import ContractPage from './pages/ContractPage'
import SquadNumbersPage from './pages/SquadNumbersPage'
import CompetitionsPage from './pages/CompetitionsPage'
import LoadGamePage from './pages/LoadGamePage'
import NationPage from './pages/NationPage'
import NationsPage from './pages/NationsPage'
import NewGamePage from './pages/NewGamePage'
import NewsPage from './pages/NewsPage'
import PersonPage from './pages/PersonPage'
import SearchPage from './pages/SearchPage'
import StartScreen from './pages/StartScreen'
import TacticsPage from './pages/TacticsPage'

// Data mostrada sem jogo aberto (início do patch CLZ 25-26)
const START_DATE = '2025-07-01'

const WEEKDAYS = ['Domingo', 'Segunda', 'Terça', 'Quarta', 'Quinta', 'Sexta', 'Sábado']

function SideButton({
  children,
  className = '',
  disabled,
  title,
  onClick,
}: {
  children: ReactNode
  className?: string
  disabled?: boolean
  title?: string
  onClick?: (e: React.MouseEvent<HTMLButtonElement>) => void
}) {
  return (
    <button
      disabled={disabled}
      title={title}
      onClick={onClick}
      data-menu-anchor
      className={`cm-side-btn min-h-[46px] ${className}`}
    >
      {children}
    </button>
  )
}

type Menu = { kind: 'competitions' | 'manager'; x: number; y: number }

/** O que o ecrã do dia de jogo precisa: o jogo, os jogos do dia e a tabela de antes. */
type MatchDayData = { fixture: Fixture; dayGames: Fixture[]; table: StandingRow[] }

/** Coluna de navegação à esquerda, como no CM 01/02. */
function SideBar({
  onOptions,
  onExit,
  notify,
  onProcessing,
  onMatch,
  matchStep,
}: {
  onOptions: () => void
  onExit: () => void
  notify: (msg: string) => void
  /** "A processar…" enquanto o Continuar Jogo corre. */
  onProcessing: (busy: boolean) => void
  /** O treinador jogou hoje: mostra o dia de jogo. */
  onMatch: (m: MatchDayData) => void
  /** Com um dia de jogo pendente, o Continuar Jogo volta a ele / faz o papel do Seguinte (não avança o dia). */
  matchStep?: () => void
}) {
  const nav = useNav()
  const game = useGame()
  const [comps, setComps] = useState<CompetitionListItem[] | null>(null)
  const [menu, setMenu] = useState<Menu | null>(null)
  const [busy, setBusy] = useState(false)
  const closeMenu = useCallback(() => setMenu(null), [])

  const browsing = game.mode === 'game' || game.mode === 'explore'
  const info = game.mode === 'game' ? game.info : null
  const [y, m, d] = (info?.date ?? START_DATE).split('-').map(Number)
  const weekday = new Date(y, m - 1, d).getDay()

  // Carrega a lista já no início para o menu abrir sem espera
  useEffect(() => {
    if (hasBackend) api.listCompetitions().then(setComps, () => {})
  }, [])

  const toggle = (kind: Menu['kind']) => (e: React.MouseEvent<HTMLButtonElement>) => {
    const r = e.currentTarget.getBoundingClientRect()
    setMenu((cur) => (cur?.kind === kind ? null : { kind, x: r.right - 2, y: r.top }))
  }

  const advance = async () => {
    if (matchStep) return matchStep()
    if (busy) return
    setBusy(true)
    onProcessing(true)
    try {
      // Com um jogo do treinador por acabar (ex.: a página recarregou), volta a ele
      let next = info
      // Convocados sem número para o jogo de amanhã: primeiro os números, como no CM
      if (next?.needsNumbers) {
        nav.go({ kind: 'squadNumbers' })
        return
      }
      if (!next?.live) {
        next = await api.continueGame()
        game.update(next)
      }
      if (next.live) {
        // O jogo do treinador é hoje: os resultados do dia só entram no fim dele, então a
        // tabela e os jogos do dia ainda estão como antes da jornada
        const { fixture } = await api.liveMatch()
        const [all, table] = await Promise.all([api.listFixtures(fixture.competitionId), api.leagueTable(fixture.competitionId)])
        onMatch({ fixture, dayGames: all.filter((f) => f.date === fixture.date), table })
      } else if (next.unreadNews > 0) {
        // Como no CM: havendo mensagens por ler, o Continuar abre a caixa de mensagens
        nav.go({ kind: 'news' })
      }
    } catch (e) {
      notify(String(e))
    } finally {
      setBusy(false)
      onProcessing(false)
    }
  }

  const managerMenu = (): MenuNode[] => {
    if (!info) return []
    const club = info.club
    return [
      { label: `Plantel ${club.shortName}`, onSelect: () => nav.go({ kind: 'club', id: club.id }) },
      { label: 'Números Plantel', onSelect: () => nav.go({ kind: 'squadNumbers' }) },
      { label: 'Tácticas', onSelect: () => nav.go({ kind: 'tactics', id: club.id }) },
      { label: 'Historial', onSelect: () => nav.go({ kind: 'clubHistory', id: club.id }) },
      'separator',
      { label: info.unreadNews ? `Notícias (${info.unreadNews})` : 'Notícias', onSelect: () => nav.go({ kind: 'news' }) },
      { label: 'Pesquisa Jogador & Staff', onSelect: () => nav.go({ kind: 'search' }) },
      { label: 'Perfil do Treinador', onSelect: () => nav.go({ kind: 'person', id: info.managerId }) },
      'separator',
      {
        label: 'Gravar Jogo',
        onSelect: () =>
          void api.saveGame().then(
            (g) => {
              game.update(g)
              notify('Jogo gravado.')
            },
            (e) => notify(String(e)),
          ),
      },
      { label: 'Sair para o Menu', onSelect: onExit },
    ]
  }

  return (
    <aside className="flex w-[86px] shrink-0 flex-col gap-1 bg-gradient-to-b from-cm-sidetop to-cm-side p-1">
      <div className="cm-side-btn min-h-[46px] flex-col font-bold text-cm-yellow">
        <span>{WEEKDAYS[weekday]}</span>
        <span>{`${d}.${m}.${String(y).slice(2)}`}</span>
      </div>
      <div className="flex gap-1">
        <button className="cm-side-btn h-9 flex-1 text-cm-yellow" disabled={!browsing || !nav.canBack} onClick={nav.back} aria-label="Voltar">
          <ArrowLeft size={16} strokeWidth={3} />
        </button>
        <button
          className="cm-side-btn h-9 flex-1 text-cm-yellow"
          disabled={!browsing || !nav.canForward}
          onClick={nav.forward}
          aria-label="Avançar"
        >
          <ArrowRight size={16} strokeWidth={3} />
        </button>
      </div>
      <SideButton disabled={!info || busy} title={info ? 'Avança até ao próximo evento' : 'Comece ou carregue um jogo'} onClick={() => void advance()}>
        Continuar Jogo
      </SideButton>
      {info ? (
        <SideButton disabled={!browsing} onClick={toggle('manager')} className="!text-cm-pink" title={info.club.name}>
          <span className="line-clamp-2 break-words">{info.managerName}</span>
        </SideButton>
      ) : (
        <SideButton
          disabled={game.mode !== 'explore'}
          onClick={() => game.setMode('start')}
          className="!text-cm-pink"
          title="Voltar ao menu inicial"
        >
          {game.mode === 'explore' ? 'Menu Inicial' : 'Treinador'}
        </SideButton>
      )}
      <SideButton disabled={!browsing} onClick={toggle('competitions')}>
        Competições
      </SideButton>
      {menu?.kind === 'competitions' && comps && (
        <CascadeMenu
          x={menu.x}
          y={menu.y}
          onClose={closeMenu}
          items={competitionsMenu(
            comps,
            (id) => nav.go({ kind: 'competition', id }),
            () => nav.go({ kind: 'competitions' }),
          )}
        />
      )}
      {menu?.kind === 'manager' && info && <CascadeMenu x={menu.x} y={menu.y} onClose={closeMenu} items={managerMenu()} />}
      <SideButton disabled={!browsing} onClick={() => nav.go({ kind: 'nations' })}>
        Nações &amp; Clubes
      </SideButton>
      <SideButton disabled={!browsing} onClick={() => nav.go({ kind: 'search' })}>
        Procurar
      </SideButton>
      <SideButton onClick={onOptions} className="!text-cm-yellow">
        Opções Jogo
      </SideButton>
      <div className="flex flex-1 items-end justify-center pb-3">
        <span className="rotate-180 font-title text-3xl font-bold tracking-widest text-cm-sidemark [writing-mode:vertical-rl]">
          NEXTCM
        </span>
      </div>
    </aside>
  )
}

function Screens() {
  const nav = useNav()
  const { current } = nav

  // Alt+← / Alt+→ e o botão "voltar" do mouse navegam no histórico
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey && e.key === 'ArrowLeft') nav.back()
      if (e.altKey && e.key === 'ArrowRight') nav.forward()
    }
    const onMouse = (e: MouseEvent) => {
      if (e.button === 3) nav.back()
      if (e.button === 4) nav.forward()
    }
    window.addEventListener('keydown', onKey)
    window.addEventListener('mouseup', onMouse)
    return () => {
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('mouseup', onMouse)
    }
  }, [nav])

  return (
    <>
      {/* A pesquisa fica montada para manter filtros, página e seleção ao voltar */}
      <div className={current.kind === 'search' ? 'h-full' : 'hidden'}>
        <SearchPage active={current.kind === 'search'} />
      </div>
      {current.kind === 'news' && <NewsPage />}
      {current.kind === 'person' && <PersonPage key={current.id} id={current.id} />}
      {current.kind === 'contract' && <ContractPage key={current.id} id={current.id} />}
      {current.kind === 'squadNumbers' && <SquadNumbersPage />}
      {current.kind === 'club' && <ClubPage key={current.id} id={current.id} />}
      {current.kind === 'tactics' && <TacticsPage key={current.id} id={current.id} />}
      {current.kind === 'clubHistory' && <ClubHistoryPage key={current.id} id={current.id} />}
      {current.kind === 'competitions' && <CompetitionsPage />}
      {current.kind === 'competition' && (
        <CompetitionPage key={`${current.id}-${current.tab ?? ''}`} id={current.id} initialTab={current.tab} />
      )}
      {current.kind === 'nations' && <NationsPage />}
      {current.kind === 'nation' && <NationPage key={current.id} id={current.id} />}
    </>
  )
}

/** Sair do jogo: oferece gravar quando há mudanças por gravar. */
function ExitDialog({ onCancel, notify }: { onCancel: () => void; notify: (msg: string) => void }) {
  const game = useGame()
  const exit = async (save: boolean) => {
    try {
      if (save) await api.saveGame()
      await game.close()
      onCancel()
    } catch (e) {
      notify(String(e))
    }
  }
  return (
    <Overlay>
      <DialogFrame title="Sair para o Menu" width={520}>
        <div className="py-3 text-center font-title text-[19px] text-white">
          {game.info?.unsaved ? 'Há alterações por gravar. Gravar o jogo antes de sair?' : 'Sair deste jogo?'}
        </div>
        <div className="flex gap-1 pb-1">
          <button className="cm-purple h-9 flex-1 font-title text-lg" onClick={onCancel}>
            Cancelar
          </button>
          {game.info?.unsaved && (
            <button className="cm-purple h-9 flex-1 font-title text-lg" onClick={() => void exit(false)}>
              Sair sem Gravar
            </button>
          )}
          <button className="cm-purple h-9 flex-1 font-title text-lg" onClick={() => void exit(!!game.info?.unsaved)}>
            {game.info?.unsaved ? 'Gravar e Sair' : 'Sair'}
          </button>
        </div>
      </DialogFrame>
    </Overlay>
  )
}

/** A direção despediu o treinador: o jogo acabou (até haver ofertas de emprego, 3.9). */
function SackedDialog({ onRead }: { onRead: () => void }) {
  const game = useGame()
  return (
    <Overlay>
      <DialogFrame title="Despedido" width={560}>
        <div className="py-3 text-center font-title text-[19px] text-white">
          A Direcção do {game.info?.club.name} dispensou-o do cargo de treinador. O jogo acabou.
        </div>
        <div className="flex gap-1 pb-1">
          <button className="cm-purple h-9 flex-1 font-title text-lg" onClick={onRead}>
            Ver Notícias
          </button>
          <button className="cm-purple h-9 flex-1 font-title text-lg" onClick={() => void game.close()}>
            Sair para o Menu
          </button>
        </div>
      </DialogFrame>
    </Overlay>
  )
}

function Shell() {
  const game = useGame()
  const [options, setOptions] = useState(false)
  const [exiting, setExiting] = useState(false)
  const [notice, setNotice] = useState<string | null>(null)
  const [processing, setProcessing] = useState(false)
  // O aviso do despedimento, até o treinador ir ler as notícias
  const [sackedSeen, setSackedSeen] = useState(false)
  const nav = useNav()
  const [match, setMatch] = useState<MatchDayData | null>(null)
  // Cada Continuar Jogo com o dia de jogo aberto avança uma fase (como o Seguinte)
  const [matchStep, setMatchStep] = useState(0)
  // Navegar pela coluna durante o dia de jogo põe-no em espera (escondido); o Continuar volta a ele
  const [matchHidden, setMatchHidden] = useState(false)
  const matchScreen = useRef<string | null>(null)
  useEffect(() => {
    const here = JSON.stringify(nav.current)
    if (!match) {
      matchScreen.current = null
      return
    }
    if (matchScreen.current === null) matchScreen.current = here
    else if (matchScreen.current !== here) setMatchHidden(true)
  }, [match, nav])
  const closeMatch = useCallback(() => {
    // Depois dos resultados do dia, como no CM: a caixa de mensagens (com a notícia do jogo)
    if (match) nav.go({ kind: 'news' })
    setMatch(null)
    setMatchHidden(false)
  }, [match, nav])

  useEffect(() => {
    if (!notice) return
    const t = setTimeout(() => setNotice(null), 1800)
    return () => clearTimeout(t)
  }, [notice])

  return (
    <div className="flex h-full">
      <SideBar
        onOptions={() => setOptions((o) => !o)}
        onExit={() => setExiting(true)}
        notify={setNotice}
        onProcessing={setProcessing}
        onMatch={setMatch}
        matchStep={
          match
            ? () => {
                if (matchHidden) {
                  // Volta ao dia de jogo onde estava, a partir da tela atual
                  matchScreen.current = JSON.stringify(nav.current)
                  setMatchHidden(false)
                } else setMatchStep((n) => n + 1)
              }
            : undefined
        }
      />
      <main className="cm-backdrop relative min-w-0 flex-1">
        {!hasBackend && (
          <div className="p-6 text-[#aab3c4]">
            Esta tela lê o banco pelo Rust. Abra pelo app desktop: <code className="text-cm-yellow">npm run tauri dev</code>
          </div>
        )}
        {hasBackend && (game.mode === 'game' || game.mode === 'explore') && <Screens />}
        {game.mode === 'start' && <StartScreen onOptions={() => setOptions(true)} />}
        {game.mode === 'newGame' && <NewGamePage />}
        {game.mode === 'loadGame' && <LoadGamePage />}
        {game.mode === 'game' && game.info?.sacked && !match && !sackedSeen && (
          <SackedDialog
            onRead={() => {
              setSackedSeen(true)
              nav.go({ kind: 'news' })
            }}
          />
        )}
        {exiting && <ExitDialog onCancel={() => setExiting(false)} notify={setNotice} />}
        {options && <OptionsDialog onClose={() => setOptions(false)} />}
        {processing && <Loading title="Continuar Jogo" detail="A processar…" delay={250} />}
        {match && game.info && !matchHidden && (
          <MatchDay
            fixture={match.fixture}
            dayGames={match.dayGames}
            table={match.table}
            own={game.info.club.id}
            step={matchStep}
            onDone={closeMatch}
          />
        )}
        {notice && (
          <div className="absolute left-1/2 top-16 z-50 -translate-x-1/2 border-2 border-black bg-cm-yellow px-4 py-1 font-title text-[17px] font-bold text-black">
            {notice}
          </div>
        )}
      </main>
    </div>
  )
}

export default function App() {
  // Ler as definições aqui faz a árvore redesenhar quando mudam (moeda, salários...)
  useSettings()
  return (
    <NavProvider>
      <GameProvider>
        <Shell />
      </GameProvider>
    </NavProvider>
  )
}
