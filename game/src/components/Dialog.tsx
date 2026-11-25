import type { ReactNode } from 'react'

// Peças dos diálogos verdes do CM (Definições do Jogo, Novo Jogo, Carregar Jogo...).

/** Verde liso e escuro dos diálogos (sem as listras de relvado). */
export const GREEN_FIELD = 'rgb(var(--c-dlgfield) / 0.6)'

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
  full,
  footer,
  onClose,
}: {
  title: ReactNode
  children: ReactNode
  header?: ReactNode
  onCancel?: () => void
  onOk?: () => void
  cancelLabel?: ReactNode
  okLabel?: ReactNode
  width?: number
  /** Ocupa toda a área (editor). */
  full?: boolean
  /** Botões próprios no lugar de Cancelar / Ok. */
  footer?: ReactNode
  /** Mostra um X no canto da faixa do título. */
  onClose?: () => void
}) {
  return (
    <div
      className="cm-dialog-in flex max-h-full max-w-full flex-col
       rounded-lg border-2 border-cm-dlgborder bg-cm-dlg shadow-[0_12px_32px_rgba(0,0,0,0.7)] p-2"
      style={full ? { width: '100%', height: '100%' } : { width }}
    >
      <div
        className="relative flex h-12 shrink-0 items-center justify-center rounded
      bg-black/40 font-sans font-bold text-2xl text-white uppercase "
      >
        {title}
        {onClose && (
          <button
            className="absolute right-2 top-1/2 flex h-8 w-8 -translate-y-1/2 items-center justify-center rounded text-xl leading-none text-cm-lilac hover:bg-white/10 hover:text-white"
            title="Fechar"
            aria-label="Fechar"
            onClick={onClose}
          >
            ✕
          </button>
        )}
      </div>
      {header}
      <div className="mt-1.5 min-h-0 flex-1 overflow-auto  px-3 py-2  bg-cm-dlgfield rounded">{children}</div>
      {footer && <div className="mt-2 flex shrink-0 gap-1">{footer}</div>}
      {!footer && (onCancel || onOk) && (
        <div className="mt-2 flex shrink-0 gap-1">
          <button className="cm-purple h-9 flex-[2] font-title text-xl" disabled={!onCancel} onClick={onCancel}>
            {cancelLabel}
          </button>
          <button className="cm-purple h-9 flex-1 font-title text-xl enabled:text-cm-yellow" disabled={!onOk} onClick={onOk}>
            {okLabel}
          </button>
        </div>
      )}
    </div>
  )
}

export function DialogRow({ label, children, wide }: { label: string; children: ReactNode; wide?: boolean }) {
  return (
    <div className="flex min-h-[30px] items-center gap-3">
      <span className={`${wide ? 'w-72' : 'w-52'} shrink-0 font-title text-[19px] leading-none text-white`}>{label}</span>
      <div className="flex flex-1 flex-wrap items-center gap-2">{children}</div>
    </div>
  )
}

/** Fundo escurecido que centra um diálogo sobre a área principal (por cima do dia de jogo, z-50). */
export function Overlay({ children }: { children: ReactNode }) {
  return <div className="absolute inset-0 z-[60] flex items-center justify-center bg-black/50 p-4">{children}</div>
}

/** Uma opção do CM que se liga e desliga com um clique: branca ligada, roxa desligada (as
 * Configurações do "Ir de Férias"). `radio` = uma de um grupo (clicar na ligada não a tira). */
export function OptionLine({
  label,
  on,
  onToggle,
  radio,
  disabled,
  children,
}: {
  label: ReactNode
  on: boolean
  onToggle: () => void
  radio?: boolean
  disabled?: boolean
  children?: ReactNode
}) {
  return (
    <div className="flex min-h-[28px] items-center gap-3 px-3">
      <button
        role={radio ? 'radio' : 'checkbox'}
        aria-checked={on}
        disabled={disabled}
        onClick={onToggle}
        className={`text-left font-title text-[17px] leading-tight ${on ? 'text-white' : 'text-cm-purplelight'}
          enabled:hover:underline disabled:opacity-50`}
      >
        {label}
      </button>
      {children}
    </div>
  )
}

/** O título de um grupo de opções ("Empregos", "Partidas"). */
export function OptionGroup({ title, radio, children }: { title: string; radio?: boolean; children: ReactNode }) {
  return (
    <div className="pb-2">
      <div className="pb-0.5 font-title text-[18px] font-bold text-cm-yellow">{title}</div>
      {/* As linhas às riscas, como as listas do CM */}
      <div role={radio ? 'radiogroup' : 'group'} aria-label={title} className="cm-zebra overflow-hidden rounded bg-black/40">
        {children}
      </div>
    </div>
  )
}
