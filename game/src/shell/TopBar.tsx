import { Play } from 'lucide-react'
import { useGame } from '../game'
import { START_DATE, WEEKDAYS } from './types'

/** "2032-02-23" -> "Segunda-Feira 23.2.32", como a data do CM. */
function barDate(iso: string): string {
  const [y, m, d] = iso.split('-').map(Number)
  const weekday = WEEKDAYS[new Date(y, m - 1, d).getDay()]
  const day = weekday.endsWith('o') ? weekday : `${weekday}-Feira`
  return `${day} ${d}.${m}.${String(y).slice(2)}`
}

/** Barra no topo da janela: a marca, a data, o clube do treinador e o Continuar Jogo. */
export default function TopBar({ onContinue, busy }: { onContinue: () => void; busy: boolean }) {
  const game = useGame()
  const info = game.mode === 'game' ? game.info : null

  return (
    <header className="flex h-11 shrink-0 items-center gap-3 border-b border-cm-line bg-cm-panel pr-2">
      <div className="flex h-full w-[86px] shrink-0 items-center justify-center gap-1.5 border-r border-cm-line">
        <img src="/favicon.svg" alt="" className="size-7" />
      </div>
      <span className="font-title text-[17px] font-bold tracking-tight">
        Next<span className="text-[#4edea3]">CM</span>
      </span>
      <span className="text-cm-tabline">|</span>
      <span className="font-bold text-cm-yellow">{barDate(info?.date ?? START_DATE)}</span>
      {info && (
        <>
          <span className="text-cm-tabline">|</span>
          <span className="truncate font-bold">{info.club.name}</span>
        </>
      )}
      <button
        className="cm-continue ml-auto"
        disabled={!info || busy}
        title={info ? 'Avança até ao próximo evento' : 'Comece ou carregue um jogo'}
        onClick={onContinue}
      >
        Continuar Jogo
        <Play size={14} strokeWidth={3} fill="currentColor" />
      </button>
    </header>
  )
}
