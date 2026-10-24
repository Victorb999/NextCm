import { GameProvider } from './game'
import { NavProvider } from './nav'
import { useSettings } from './settings'
import Shell from './shell/Shell'
import Stage from './shell/Stage'

// A raiz: os providers (navegação e jogo aberto) e a janela. As peças da janela estão em
// `shell/`: o palco 16:9 (`Stage`), a coluna (`SideBar`), as telas (`Screens`), os diálogos
// e o dia de jogo.

export default function App() {
  // Ler as definições aqui faz a árvore redesenhar quando mudam (moeda, salários...)
  useSettings()
  return (
    <NavProvider>
      <GameProvider>
        <Stage>
          <Shell />
        </Stage>
      </GameProvider>
    </NavProvider>
  )
}
