import type { ManagerDialog } from './managerMenu'
import { Globe, Search, Settings, Trophy, User } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api, hasBackend } from '../api'
import type { CompetitionListItem } from '../bindings/CompetitionListItem'
import CascadeMenu from '../components/CascadeMenu'
import { competitionsMenu } from '../competitionsMenu'
import { useGame } from '../game'
import { useNav } from '../nav'
import type { Screen } from '../nav'
import { managerMenu } from './managerMenu'

type Menu = { kind: 'competitions' | 'manager'; x: number; y: number }
type Section = 'manager' | 'competitions' | 'nations' | 'search'

function SideButton({
  children,
  icon,
  className = '',
  active,
  disabled,
  title,
  onClick,
}: {
  children: ReactNode
  icon: ReactNode
  className?: string
  /** A tela aberta é deste botão (ou o menu dele está aberto). */
  active?: boolean
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
      aria-current={active || undefined}
      className={`cm-side-btn min-h-[44px] overflow-hidden ${className}`}
    >
      <span className="shrink-0">{icon}</span>
      {children}
    </button>
  )
}

/** ◀ ▶: o histórico de telas. */
function HistoryButtons({ enabled }: { enabled: boolean }) {
  const nav = useNav()
  return (
    <div className="flex gap-1.5">
      <button className="cm-side-btn h-8 justify-center text-cm-yellow" disabled={!enabled || !nav.canBack} onClick={nav.back} aria-label="Voltar">
        ◀
      </button>
      <button className="cm-side-btn h-8 justify-center text-cm-yellow" disabled={!enabled || !nav.canForward} onClick={nav.forward} aria-label="Avançar">
        ▶
      </button>
    </div>
  )
}

/** O botão da coluna a que a tela aberta pertence (só para a marcar). */
function sectionOf(s: Screen, club: number | undefined, manager: number | undefined): Section | null {
  switch (s.kind) {
    case 'competitions':
    case 'competition':
      return 'competitions'
    case 'nations':
    case 'nation':
      return 'nations'
    case 'search':
      return 'search'
    case 'news':
    case 'board':
    case 'jobs':
    case 'worldTransfers':
    case 'career':
    case 'squadNumbers':
      return 'manager'
    case 'club':
    case 'tactics':
    case 'clubHistory':
      return s.id === club ? 'manager' : null
    case 'person':
      return s.id === manager ? 'manager' : null
    default:
      return null
  }
}

/** Coluna de navegação à esquerda, como no CM 01/02 (a data e o Continuar Jogo estão na barra do topo). */
export default function SideBar({
  onOptions,
  onExit,
  onDialog,
}: {
  onOptions: () => void
  onExit: () => void
  /** Os diálogos do menu do treinador (Gravar, Férias, Rescindir, Retirar-se). */
  onDialog: (d: ManagerDialog) => void
}) {
  const nav = useNav()
  const game = useGame()
  const [comps, setComps] = useState<CompetitionListItem[] | null>(null)
  const [menu, setMenu] = useState<Menu | null>(null)
  const closeMenu = useCallback(() => setMenu(null), [])

  const browsing = game.mode === 'game' || game.mode === 'explore'
  const info = game.mode === 'game' ? game.info : null
  const section = browsing ? sectionOf(nav.current, info?.club.id, info?.managerId) : null
  const marked = (s: Section) => (menu ? menu.kind === s : section === s)

  // Carrega a lista já no início para o menu abrir sem espera
  useEffect(() => {
    if (hasBackend) api.listCompetitions().then(setComps, () => {})
  }, [])

  const toggle = (kind: Menu['kind']) => (e: React.MouseEvent<HTMLButtonElement>) => {
    const r = e.currentTarget.getBoundingClientRect()
    setMenu((cur) => (cur?.kind === kind ? null : { kind, x: r.right + 2, y: r.top }))
  }

  return (
    <aside className="flex w-[150px] shrink-0 flex-col gap-1.5 border-r border-cm-line bg-cm-sidedark p-2">
      <HistoryButtons enabled={browsing} />
      {info ? (
        <SideButton
          icon={<User size={16} />}
          disabled={!browsing}
          active={marked('manager')}
          onClick={toggle('manager')}
          className="relative !text-cm-pink"
          title={info.club.name}
        >
          <span className="line-clamp-2 break-words">{info.managerName}</span>
          <span className="ml-auto shrink-0 text-[10px] text-white/40">▶</span>
          {info.unreadNews > 0 && (
            <span className="absolute -right-1.5 -top-1.5 flex h-[18px] min-w-[18px] items-center justify-center rounded-full bg-cm-sel px-1 text-[10px] font-bold text-white">
              {info.unreadNews}
            </span>
          )}
        </SideButton>
      ) : (
        <SideButton
          icon={<User size={16} />}
          disabled={game.mode !== 'explore'}
          onClick={() => game.setMode('start')}
          className="!text-cm-pink"
          title="Voltar ao menu inicial"
        >
          {game.mode === 'explore' ? 'Menu Inicial' : 'Treinador'}
        </SideButton>
      )}
      <SideButton icon={<Trophy size={16} />} disabled={!browsing} active={marked('competitions')} onClick={toggle('competitions')}>
        <span className="min-w-0 truncate">Competições</span>
        <span className="ml-auto shrink-0 text-[10px] text-white/40">▶</span>
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
          items={managerMenu({ info, go: nav.go, onExit, onDialog })}
        />
      )}
      <SideButton icon={<Globe size={16} />} disabled={!browsing} active={marked('nations')} onClick={() => nav.go({ kind: 'nations' })}>
        Nações &amp; Clubes
      </SideButton>
      <SideButton icon={<Search size={16} />} disabled={!browsing} active={marked('search')} onClick={() => nav.go({ kind: 'search' })}>
        Procurar
      </SideButton>
      <SideButton icon={<Settings size={16} />} onClick={onOptions} className="!text-cm-yellow">
        Opções Jogo
      </SideButton>
    </aside>
  )
}
