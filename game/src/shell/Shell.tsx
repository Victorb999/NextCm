import { useState } from 'react'
import { hasBackend } from '../api'
import Loading from '../components/Loading'
import MatchDay from '../components/MatchDay'
import OptionsDialog from '../components/OptionsDialog'
import { useGame } from '../game'
import { useNav } from '../nav'
import LoadGamePage from '../pages/LoadGamePage'
import NewGamePage from '../pages/NewGamePage'
import StartScreen from '../pages/StartScreen'
import ExitDialog from './ExitDialog'
import Notice from './Notice'
import SackedDialog from './SackedDialog'
import Screens from './Screens'
import SideBar from './SideBar'
import { useMatchDay } from './useMatchDay'
import { useNotice } from './useNotice'

/** A janela do jogo: a coluna à esquerda e, ao lado, a tela do modo atual e o que vem por cima. */
export default function Shell() {
  const game = useGame()
  const nav = useNav()
  const [options, setOptions] = useState(false)
  const [exiting, setExiting] = useState(false)
  const [processing, setProcessing] = useState(false)
  // O aviso do despedimento, até o treinador ir ler as notícias
  const [sackedSeen, setSackedSeen] = useState(false)
  const { notice, notify } = useNotice()
  const matchDay = useMatchDay()
  const browsing = game.mode === 'game' || game.mode === 'explore'

  return (
    <div className="flex h-full">
      <SideBar
        onOptions={() => setOptions((o) => !o)}
        onExit={() => setExiting(true)}
        notify={notify}
        onProcessing={setProcessing}
        onMatch={matchDay.open}
        matchStep={matchDay.advance}
      />
      <main className="cm-backdrop relative min-w-0 flex-1">
        {!hasBackend && (
          <div className="p-6 text-[#aab3c4]">
            Esta tela lê o banco pelo Rust. Abra pelo app desktop: <code className="text-cm-yellow">npm run tauri dev</code>
          </div>
        )}
        {hasBackend && browsing && <Screens />}
        {game.mode === 'start' && <StartScreen onOptions={() => setOptions(true)} />}
        {game.mode === 'newGame' && <NewGamePage />}
        {game.mode === 'loadGame' && <LoadGamePage />}
        {game.mode === 'game' && game.info?.sacked && !matchDay.match && !sackedSeen && (
          <SackedDialog
            onRead={() => {
              setSackedSeen(true)
              nav.go({ kind: 'news' })
            }}
          />
        )}
        {exiting && <ExitDialog onCancel={() => setExiting(false)} notify={notify} />}
        {options && <OptionsDialog onClose={() => setOptions(false)} />}
        {processing && <Loading title="Continuar Jogo" detail="A processar…" delay={250} />}
        {matchDay.match && game.info && !matchDay.hidden && (
          <MatchDay
            fixture={matchDay.match.fixture}
            dayGames={matchDay.match.dayGames}
            table={matchDay.match.table}
            own={game.info.club.id}
            step={matchDay.step}
            onDone={matchDay.close}
          />
        )}
        {notice && <Notice text={notice} />}
      </main>
    </div>
  )
}
