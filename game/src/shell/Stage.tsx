import type { ReactNode } from 'react'

/**
 * O palco do jogo, sempre em 16:9 (a proporção das resoluções das Opções Jogo): ocupa o
 * maior retângulo 16:9 que cabe na janela, ao centro, com barras pretas no que sobra (no
 * ecrã completo de um monitor 16:10 ou 4:3, por exemplo). Assim o layout nunca estica.
 * O `#stage` recebe a tela da partida (por portal), que cobre a coluna e tem de ficar
 * dentro das barras pretas. `overflow-clip` e não `hidden`: o hidden ainda rola por código
 * (um foco), e a tela ficava deslocada.
 */
export default function Stage({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full w-full items-center justify-center bg-black">
      <div id="stage" className="relative overflow-clip" style={{ aspectRatio: '16 / 9', width: 'min(100vw, calc(100vh * 16 / 9))' }}>
        {children}
      </div>
    </div>
  )
}
