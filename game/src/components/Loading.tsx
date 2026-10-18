import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'

/**
 * Ecrã de carregamento no estilo do CM: caixa preta com o título e a barra amarela a correr.
 * `delay` evita que pisque quando a operação é rápida.
 */
export default function Loading({ title, detail, delay = 0 }: { title: ReactNode; detail?: ReactNode; delay?: number }) {
  const [visible, setVisible] = useState(delay === 0)
  useEffect(() => {
    if (delay === 0) return
    const t = setTimeout(() => setVisible(true), delay)
    return () => clearTimeout(t)
  }, [delay])
  if (!visible) return null
  return (
    <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="w-[440px] border-2 border-[#1d4a20] bg-[#0b3a10] p-2 shadow-[4px_4px_0_rgba(0,0,0,0.6)]">
        <div className="flex h-11 items-center justify-center border-2 border-[#333] bg-black font-title text-xl text-white">
          {title}
        </div>
        <div className="mt-2 px-1 text-center text-cm-yellow">{detail ?? 'Por favor aguarde…'}</div>
        <div className="mx-1 my-2 h-4 border border-black bg-black p-0.5">
          <div className="cm-progress h-full w-full" />
        </div>
      </div>
    </div>
  )
}
