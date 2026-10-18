import type { ReactNode } from 'react'

// Peças dos diálogos verdes do CM (Definições do Jogo, Novo Jogo, Carregar Jogo...).

export const GREEN_FIELD = 'repeating-linear-gradient(90deg, #0f5a16 0 70px, #126a1a 70px 140px)'

/** Moldura: título preto, área verde e os botões Cancelar / Ok. */
export function DialogFrame({
  title,
  children,
  header,
  onCancel,
  onOk,
  cancelLabel = 'Cancelar',
  okLabel = 'Ok',
  width = 660,
}: {
  title: ReactNode
  children: ReactNode
  header?: ReactNode
  onCancel?: () => void
  onOk?: () => void
  cancelLabel?: ReactNode
  okLabel?: ReactNode
  width?: number
}) {
  return (
    <div
      className="flex max-h-full max-w-full flex-col border-2 border-[#1d4a20] bg-[#0b3a10] p-2 shadow-[4px_4px_0_rgba(0,0,0,0.6)]"
      style={{ width }}
    >
      <div className="flex h-12 shrink-0 items-center justify-center border-2 border-[#333] bg-black font-title text-2xl text-white">
        {title}
      </div>
      {header}
      <div className="mt-1.5 min-h-0 flex-1 overflow-auto border-2 border-[#1d3a1f] px-3 py-2" style={{ background: GREEN_FIELD }}>
        {children}
      </div>
      {(onCancel || onOk) && (
        <div className="mt-2 flex shrink-0 gap-1">
          <button className="cm-purple h-9 flex-[2] font-title text-xl" disabled={!onCancel} onClick={onCancel}>
            {cancelLabel}
          </button>
          <button className="cm-purple h-9 flex-1 font-title text-xl" disabled={!onOk} onClick={onOk}>
            {okLabel}
          </button>
        </div>
      )}
    </div>
  )
}

export function DialogRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex min-h-[30px] items-center gap-3">
      <span className="w-52 shrink-0 font-title text-[19px] leading-none text-white">{label}</span>
      <div className="flex flex-1 flex-wrap items-center gap-2">{children}</div>
    </div>
  )
}

/** Fundo escurecido que centra um diálogo sobre a área principal. */
export function Overlay({ children }: { children: ReactNode }) {
  return <div className="absolute inset-0 z-40 flex items-center justify-center bg-black/50 p-4">{children}</div>
}
