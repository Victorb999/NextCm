import { ArrowLeft, ArrowRight } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api, hasBackend } from './api'
import type { CompetitionListItem } from './bindings/CompetitionListItem'
import CascadeMenu from './components/CascadeMenu'
import { competitionsMenu } from './competitionsMenu'
import { NavProvider, useNav } from './nav'
import ClubHistoryPage from './pages/ClubHistoryPage'
import ClubPage from './pages/ClubPage'
import CompetitionPage from './pages/CompetitionPage'
import CompetitionsPage from './pages/CompetitionsPage'
import NationPage from './pages/NationPage'
import NationsPage from './pages/NationsPage'
import PersonPage from './pages/PersonPage'
import SearchPage from './pages/SearchPage'
import TacticsPage from './pages/TacticsPage'

// Data inicial do jogo (patch CLZ 25-26). Passa a vir do save na Fase 3.
const GAME_DATE = new Date(2025, 6, 1)

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

/** Coluna de navegação à esquerda, como no CM 01/02. */
function SideBar() {
  const nav = useNav()
  const d = GAME_DATE
  const [comps, setComps] = useState<CompetitionListItem[] | null>(null)
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null)
  const closeMenu = useCallback(() => setMenu(null), [])

  // Carrega a lista já no início para o menu abrir sem espera
  useEffect(() => {
    if (hasBackend) api.listCompetitions().then(setComps, () => {})
  }, [])
  return (
    <aside className="flex w-[86px] shrink-0 flex-col gap-1 bg-gradient-to-b from-[#4f5d77] to-cm-side p-1">
      <div className="cm-side-btn min-h-[46px] flex-col font-bold text-cm-yellow">
        <span>{WEEKDAYS[d.getDay()]}</span>
        <span>{`${d.getDate()}.${d.getMonth() + 1}.${String(d.getFullYear()).slice(2)}`}</span>
      </div>
      <div className="flex gap-1">
        <button className="cm-side-btn h-9 flex-1 text-cm-yellow" disabled={!nav.canBack} onClick={nav.back} aria-label="Voltar">
          <ArrowLeft size={16} strokeWidth={3} />
        </button>
        <button
          className="cm-side-btn h-9 flex-1 text-cm-yellow"
          disabled={!nav.canForward}
          onClick={nav.forward}
          aria-label="Avançar"
        >
          <ArrowRight size={16} strokeWidth={3} />
        </button>
      </div>
      <SideButton disabled title="Disponível na Fase 3">
        Continuar Jogo
      </SideButton>
      <SideButton disabled title="Disponível na Fase 3" className="!text-cm-pink">
        Treinador
      </SideButton>
      <SideButton
        onClick={(e) => {
          const r = e.currentTarget.getBoundingClientRect()
          setMenu((m) => (m ? null : { x: r.right - 2, y: r.top }))
        }}
      >
        Competições
      </SideButton>
      {menu && comps && (
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
      <SideButton onClick={() => nav.go({ kind: 'nations' })}>Nações &amp; Clubes</SideButton>
      <SideButton onClick={() => nav.go({ kind: 'search' })}>Procurar</SideButton>
      <SideButton disabled title="Em breve" className="!text-cm-yellow">
        Opções Jogo
      </SideButton>
      <div className="flex flex-1 items-end justify-center pb-3">
        <span className="rotate-180 font-title text-3xl font-bold tracking-widest text-[#5f6c84] [writing-mode:vertical-rl]">
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
      {current.kind === 'person' && <PersonPage key={current.id} id={current.id} />}
      {current.kind === 'club' && <ClubPage key={current.id} id={current.id} />}
      {current.kind === 'tactics' && <TacticsPage key={current.id} id={current.id} />}
      {current.kind === 'clubHistory' && <ClubHistoryPage key={current.id} id={current.id} />}
      {current.kind === 'competitions' && <CompetitionsPage />}
      {current.kind === 'competition' && <CompetitionPage key={current.id} id={current.id} />}
      {current.kind === 'nations' && <NationsPage />}
      {current.kind === 'nation' && <NationPage key={current.id} id={current.id} />}
    </>
  )
}

export default function App() {
  return (
    <NavProvider>
      <div className="flex h-full">
        <SideBar />
        <main className="cm-backdrop min-w-0 flex-1">
          {hasBackend ? (
            <Screens />
          ) : (
            <div className="p-6 text-[#aab3c4]">
              Esta tela lê o banco pelo Rust. Abra pelo app desktop: <code className="text-cm-yellow">npm run tauri dev</code>
            </div>
          )}
        </main>
      </div>
    </NavProvider>
  )
}
