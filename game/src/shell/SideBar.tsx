import type { LineupCheck } from '../bindings/LineupCheck'
import { ArrowLeft, ArrowRight } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api, hasBackend } from '../api'
import type { CompetitionListItem } from '../bindings/CompetitionListItem'
import CascadeMenu from '../components/CascadeMenu'
import { competitionsMenu } from '../competitionsMenu'
import { useGame } from '../game'
import { useNav } from '../nav'
import { managerMenu } from './managerMenu'
import type { MatchDayData } from './types'
import { START_DATE, WEEKDAYS } from './types'
import { useContinueGame } from './useContinueGame'

type Menu = { kind: 'competitions' | 'manager'; x: number; y: number }

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
    <button disabled={disabled} title={title} onClick={onClick} data-menu-anchor className={`cm-side-btn min-h-[46px] ${className}`}>
      {children}
    </button>
  )
}

/** A data do jogo no topo da coluna ("Terça / 1.7.25"). */
function DateBox({ date }: { date: string }) {
  const [y, m, d] = date.split('-').map(Number)
  const weekday = new Date(y, m - 1, d).getDay()
  return (
    <div className="cm-side-btn min-h-[46px] flex-col font-bold text-cm-yellow">
      <span>{WEEKDAYS[weekday]}</span>
      <span>{`${d}.${m}.${String(y).slice(2)}`}</span>
    </div>
  )
}

/** ◀ ▶: o histórico de telas. */
function HistoryButtons({ enabled }: { enabled: boolean }) {
  const nav = useNav()
  return (
    <div className="flex gap-1">
      <button className="cm-side-btn h-9 flex-1 text-cm-yellow" disabled={!enabled || !nav.canBack} onClick={nav.back} aria-label="Voltar">
        <ArrowLeft size={16} strokeWidth={3} />
      </button>
      <button className="cm-side-btn h-9 flex-1 text-cm-yellow" disabled={!enabled || !nav.canForward} onClick={nav.forward} aria-label="Avançar">
        <ArrowRight size={16} strokeWidth={3} />
      </button>
    </div>
  )
}

/** Coluna de navegação à esquerda, como no CM 01/02. */
export default function SideBar({
  onOptions,
  onExit,
  notify,
  onProcessing,
  onMatch,
  matchStep,
  onLineup,
}: {
  onOptions: () => void
  onExit: () => void
  notify: (msg: string) => void
  /** "A processar…" enquanto o Continuar Jogo corre. */
  onProcessing: (busy: boolean) => void
  /** O treinador jogou hoje: mostra o dia de jogo. */
  onMatch: (m: MatchDayData) => void
  /** Com um dia de jogo pendente, o Continuar Jogo volta a ele / faz o papel do Seguinte. */
  matchStep?: () => void
  onLineup: (check: LineupCheck, go: () => void) => void
}) {
  const nav = useNav()
  const game = useGame()
  const [comps, setComps] = useState<CompetitionListItem[] | null>(null)
  const [menu, setMenu] = useState<Menu | null>(null)
  const closeMenu = useCallback(() => setMenu(null), [])
  const { advance, busy } = useContinueGame({ notify, onProcessing, onMatch, matchStep, onLineup })

  const browsing = game.mode === 'game' || game.mode === 'explore'
  const info = game.mode === 'game' ? game.info : null

  // Carrega a lista já no início para o menu abrir sem espera
  useEffect(() => {
    if (hasBackend) api.listCompetitions().then(setComps, () => {})
  }, [])

  const toggle = (kind: Menu['kind']) => (e: React.MouseEvent<HTMLButtonElement>) => {
    const r = e.currentTarget.getBoundingClientRect()
    setMenu((cur) => (cur?.kind === kind ? null : { kind, x: r.right - 2, y: r.top }))
  }

  return (
    <aside className="flex w-[86px] shrink-0 flex-col gap-1 bg-gradient-to-b from-cm-sidetop to-cm-side p-1">
      <DateBox date={info?.date ?? START_DATE} />
      <HistoryButtons enabled={browsing} />
      <SideButton disabled={!info || busy} title={info ? 'Avança até ao próximo evento' : 'Comece ou carregue um jogo'} onClick={() => void advance()}>
        Continuar Jogo
      </SideButton>
      {info ? (
        <SideButton disabled={!browsing} onClick={toggle('manager')} className="!text-cm-pink" title={info.club.name}>
          <span className="line-clamp-2 break-words">{info.managerName}</span>
        </SideButton>
      ) : (
        <SideButton disabled={game.mode !== 'explore'} onClick={() => game.setMode('start')} className="!text-cm-pink" title="Voltar ao menu inicial">
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
      {menu?.kind === 'manager' && info && (
        <CascadeMenu
          x={menu.x}
          y={menu.y}
          onClose={closeMenu}
          items={managerMenu({ info, go: nav.go, update: game.update, notify, onExit })}
        />
      )}
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
        <span className="rotate-180 font-title text-3xl font-bold tracking-widest text-cm-sidemark [writing-mode:vertical-rl]">NEXTCM</span>
      </div>
    </aside>
  )
}
