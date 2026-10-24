import type { ReactNode } from 'react'

/**
 * O palco do jogo, sempre em 16:9 (a proporção das resoluções das Opções Jogo): ocupa o
 * maior retângulo 16:9 que cabe na janela, ao centro, com barras pretas no que sobra (no
 * ecrã completo de um monitor 16:10 ou 4:3, por exemplo). Assim o layout nunca estica.
 */
export default function Stage({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full w-full items-center justify-center bg-black">
      <div className="relative overflow-hidden" style={{ aspectRatio: '16 / 9', width: 'min(100vw, calc(100vh * 16 / 9))' }}>
        {children}
      </div>
    </div>
  )
}
