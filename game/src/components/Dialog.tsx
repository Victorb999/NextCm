import type { ReactNode } from "react"

// Peças dos diálogos verdes do CM (Definições do Jogo, Novo Jogo, Carregar Jogo...).

/** Verde liso e escuro dos diálogos (sem as listras de relvado). */
export const GREEN_FIELD = "#01322099"

/** Moldura: título preto, área verde e os botões Cancelar / Ok. */
export function DialogFrame({
  title,
  children,
  header,
  onCancel,
  onOk,
  cancelLabel = "Cancelar",
  okLabel = "Ok",
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
      className="flex max-h-full max-w-full flex-col 
       rounded-lg border-2 border-[#10b981] bg-[#0f241b] shadow-[0_12px_32px_rgba(0,0,0,0.7)] p-2"
      style={{ width }}
    >
      <div
        className="flex h-12 shrink-0 items-center justify-center rounded
      bg-[#01010166] font-sans font-bold text-2xl text-white uppercase "
      >
        {title}
      </div>
      {header}
      <div className="mt-1.5 min-h-0 flex-1 overflow-auto  px-3 py-2  bg-[#013220ff] rounded">
        {children}
      </div>
      {(onCancel || onOk) && (
        <div className="mt-2 flex shrink-0 gap-1">
          <button
            className="cm-purple h-9 flex-[2] font-title text-xl"
            disabled={!onCancel}
            onClick={onCancel}
          >
            {cancelLabel}
          </button>
          <button
            className="cm-purple h-9 flex-1 font-title text-xl"
            disabled={!onOk}
            onClick={onOk}
          >
            {okLabel}
          </button>
        </div>
      )}
    </div>
  )
}

export function DialogRow({
  label,
  children,
  wide,
}: {
  label: string
  children: ReactNode
  wide?: boolean
}) {
  return (
    <div className="flex min-h-[30px] items-center gap-3">
      <span
        className={`${wide ? "w-72" : "w-52"} shrink-0 font-title text-[19px] leading-none text-white`}
      >
        {label}
      </span>
      <div className="flex flex-1 flex-wrap items-center gap-2">{children}</div>
    </div>
  )
}

/** Fundo escurecido que centra um diálogo sobre a área principal. */
export function Overlay({ children }: { children: ReactNode }) {
  return (
    <div className="absolute inset-0 z-40 flex items-center justify-center bg-black/50 p-4">
      {children}
    </div>
  )
}
