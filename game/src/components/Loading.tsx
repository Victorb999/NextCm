import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'

/**
 * Ecrã de carregamento no estilo do CM: caixa preta com o título e a barra amarela a correr.
 * `delay` evita que pisque quando a operação é rápida.
 */
export default function Loading({
  title,
  detail,
  delay = 0,
  footer,
  progress,
}: {
  title: ReactNode
  detail?: ReactNode
  delay?: number
  /** A barra de baixo (os resultados do dia no "Processando", como no CM). */
  footer?: ReactNode
  /** Quanto já fez (0 a 1): a barra enche até aí; sem isto, a barra listrada de sempre. */
  progress?: number
}) {
  const [visible, setVisible] = useState(delay === 0)
  useEffect(() => {
    if (delay === 0) return
    const t = setTimeout(() => setVisible(true), delay)
    return () => clearTimeout(t)
  }, [delay])
  if (!visible) return null
  return (
    <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/50">
      {/* A moldura dos diálogos verdes (DialogFrame): título escuro e a área verde */}
      <div className="w-[460px] rounded-lg border-2 border-cm-dlgborder bg-cm-dlg p-2 shadow-[0_12px_32px_rgba(0,0,0,0.7)]">
        <div className="flex h-12 items-center justify-center rounded bg-[#01010166] font-sans text-2xl font-bold uppercase text-white">
          {title}
        </div>
        <div className="mt-1.5 rounded bg-[#013220ff] px-3 py-3">
          <div className="text-center text-[16px] text-cm-yellow">{detail ?? 'Por favor aguarde…'}</div>
          <div className="mt-2.5 h-4 overflow-hidden rounded border border-cm-dlgline bg-black p-0.5">
            <div
              className="cm-progress h-full rounded-sm"
              style={{ width: progress === undefined ? '100%' : `${Math.max(3, progress * 100)}%` }}
            />
          </div>
        </div>
      </div>
      {footer && (
        <div className="absolute inset-x-0 bottom-0 flex h-8 items-center overflow-hidden whitespace-nowrap border-t border-cm-line bg-black/90 px-3 text-[13px] font-semibold text-cm-yellow">
          {footer}
        </div>
      )}
    </div>
  )
}
