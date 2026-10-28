import type { ManagerDialog } from './managerMenu'
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
      <HistoryButtons enabled={browsing} />
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
          items={managerMenu({ info, go: nav.go, onExit, onDialog })}
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
    </aside>
  )
}
