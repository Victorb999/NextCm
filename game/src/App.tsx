import { ArrowLeft, ArrowRight } from 'lucide-react'
import type { ReactNode } from 'react'
import { inTauri } from './api'
import SearchPage from './pages/SearchPage'

// Data inicial do jogo (patch CLZ 25-26). Passa a vir do save na Fase 3.
const GAME_DATE = new Date(2025, 6, 1)

const WEEKDAYS = ['Domingo', 'Segunda', 'Terça', 'Quarta', 'Quinta', 'Sexta', 'Sábado']

function SideButton({ children, className = '', disabled, title }: { children: ReactNode; className?: string; disabled?: boolean; title?: string }) {
  return (
    <button disabled={disabled} title={title} className={`cm-side-btn min-h-[46px] ${className}`}>
      {children}
    </button>
  )
}

/** Coluna de navegação à esquerda, como no CM 01/02. */
function SideBar() {
  const d = GAME_DATE
  return (
    <aside className="flex w-[86px] shrink-0 flex-col gap-1 bg-gradient-to-b from-[#4f5d77] to-cm-side p-1">
      <div className="cm-side-btn min-h-[46px] flex-col font-bold text-cm-yellow">
        <span>{WEEKDAYS[d.getDay()]}</span>
        <span>{`${d.getDate()}.${d.getMonth() + 1}.${String(d.getFullYear()).slice(2)}`}</span>
      </div>
      <div className="flex gap-1">
        <button className="cm-side-btn h-9 flex-1 text-cm-yellow" disabled aria-label="Voltar">
          <ArrowLeft size={16} strokeWidth={3} />
        </button>
        <button className="cm-side-btn h-9 flex-1 text-cm-yellow" disabled aria-label="Avançar">
          <ArrowRight size={16} strokeWidth={3} />
        </button>
      </div>
      <SideButton disabled title="Disponível na Fase 3">
        Continuar Jogo
      </SideButton>
      <SideButton disabled title="Disponível na Fase 3" className="!text-cm-pink">
        Treinador
      </SideButton>
      <SideButton disabled title="Em breve">
        Competições
      </SideButton>
      <SideButton disabled title="Em breve">
        Nações &amp; Clubes
      </SideButton>
      <SideButton>Procurar</SideButton>
      <SideButton disabled title="Em breve" className="!text-cm-yellow">
        Opções Jogo
      </SideButton>
      <div className="flex flex-1 items-end justify-center pb-3">
        <span className="font-title text-3xl font-bold tracking-widest text-[#5f6c84] [writing-mode:vertical-rl] rotate-180">
          NEXTCM
        </span>
      </div>
    </aside>
  )
}

export default function App() {
  return (
    <div className="flex h-full">
      <SideBar />
      <main className="cm-backdrop min-w-0 flex-1">
        {inTauri ? (
          <SearchPage />
        ) : (
          <div className="p-6 text-[#aab3c4]">
            Esta tela lê o banco pelo Rust. Abra pelo app desktop: <code className="text-cm-yellow">npm run tauri dev</code>
          </div>
        )}
      </main>
    </div>
  )
}
