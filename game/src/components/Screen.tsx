import { ChevronDown } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'

/** Luminância aproximada de uma cor `#rrggbb` (0 = preto, 1 = branco). */
function luminance(hex: string) {
  const n = parseInt(hex.slice(1, 7), 16)
  if (Number.isNaN(n)) return 0.5
  return (0.299 * (n >> 16) + 0.587 * ((n >> 8) & 255) + 0.114 * (n & 255)) / 255
}

// Como no CM: letra clara ganha contorno preto; letra escura, sombra cinza
const OUTLINE = '-1px -1px 0 #000, 1px -1px 0 #000, -1px 1px 0 #000, 1px 1px 0 #000, 2px 2px 0 #000'
const DROP = '2px 2px 0 rgba(0,0,0,0.3)'

/** Faixa de título das telas do CM: caixa colorida com o nome grande centralizado. */
export function TitleBand({ title, fg, bg }: { title: ReactNode; fg: string; bg: string }) {
  return (
    <div className="shrink-0 p-1.5 pb-1">
      <div
        className="flex h-14 items-center justify-center border-2 border-black font-title text-[30px] font-bold tracking-tight"
        style={{ color: fg, background: bg, textShadow: luminance(fg) > 0.5 ? OUTLINE : DROP }}
      >
        {title}
      </div>
    </div>
  )
}

/** Cabeçalho das telas no visual novo: emblema (ou ícone), etiqueta, título grande e algo à direita. */
export function ScreenHeader({
  title,
  icon,
  emblem,
  eyebrow,
  right,
}: {
  title: ReactNode
  icon?: ReactNode
  /** O emblema do clube: iniciais nas cores dele. */
  emblem?: { text: string; fg: string; bg: string }
  /** Linha pequena por cima do título (etiquetas, época). */
  eyebrow?: ReactNode
  right?: ReactNode
}) {
  return (
    <div className="mx-2.5 mt-2.5 flex shrink-0 items-center gap-3 rounded-md bg-[#1a1b21] px-4 py-2.5">
      {emblem && (
        <span
          className="flex size-11 shrink-0 items-center justify-center rounded font-title text-[18px] font-bold"
          style={{ color: emblem.fg, background: emblem.bg }}
        >
          {emblem.text}
        </span>
      )}
      {icon && <span className="shrink-0 text-[#c8c8d0]">{icon}</span>}
      <div className="min-w-0">
        {eyebrow && <div className="flex items-center gap-2 text-[11px] font-bold font-mono uppercase tracking-[0.12em] text-[#c8c8d0]">{eyebrow}</div>}
        <div className="truncate font-title text-[24px] font-bold leading-tight text-white">{title}</div>
      </div>
      <div className="ml-auto flex shrink-0 items-center gap-2">{right}</div>
    </div>
  )
}

export type Tab = { key: string; label: string; disabled?: boolean }

/** Linha de abas do CM: escuras, a selecionada com borda amarela. */
export function Tabs({ tabs, active, onChange }: { tabs: Tab[]; active: string; onChange: (key: string) => void }) {
  return (
    <div className="flex shrink-0 gap-1.5 px-2.5" role="tablist">
      {tabs.map((t) => (
        <button
          key={t.key}
          role="tab"
          aria-selected={t.key === active}
          disabled={t.disabled}
          onClick={() => onChange(t.key)}
          className="cm-tab"
        >
          {t.label}
        </button>
      ))}
    </div>
  )
}

/** Barra inferior roxa com Atrás / Seguinte (ou Cancelar / Ok). */
export function BottomBar({
  left = 'Atrás',
  right = 'Seguinte',
  onLeft,
  onRight,
}: {
  left?: ReactNode
  right?: ReactNode
  onLeft?: () => void
  onRight?: () => void
}) {
  return (
    <div className="flex h-[50px] shrink-0 gap-1.5 px-2.5 pb-2.5 pt-1.5">
      <button className="cm-purple flex-[2] font-title text-lg" disabled={!onLeft} onClick={onLeft}>
        {left}
      </button>
      <button className="cm-purple flex-1 font-title text-lg" disabled={!onRight} onClick={onRight}>
        {right}
      </button>
    </div>
  )
}

/** Botão roxo com menu ("Vêr ▼", "Critério ▼"), como os do CM. */
export function PurpleMenu<T extends string>({
  label,
  options,
  value,
  onChange,
  disabled,
  align = 'left',
  trigger,
  buttonClass,
}: {
  label: string
  options: { key: T; label: string }[]
  value?: T
  onChange?: (key: T) => void
  disabled?: boolean
  /** Lado por onde a lista abre; "right" para botões encostados à direita do ecrã. */
  align?: 'left' | 'right'
  /** Conteúdo do botão no lugar do rótulo roxo (cabeçalho das Tácticas); abre igual, com um clique. */
  trigger?: ReactNode
  buttonClass?: string
}) {
  const [open, setOpen] = useState(false)
  // Com a lista aberta, o Escape só a fecha (não volta à tela anterior)
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      e.stopImmediatePropagation()
      setOpen(false)
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [open])
  return (
    <div className="relative" onMouseLeave={() => setOpen(false)}>
      <button
        className={buttonClass ?? 'cm-purple flex h-6 w-40 items-center justify-center gap-1'}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
        aria-label={trigger ? label : undefined}
      >
        {trigger ?? (
          <>
            {label} <ChevronDown size={14} />
          </>
        )}
      </button>
      {open && (
        <div
          className={`absolute top-full z-30 min-w-40 overflow-hidden whitespace-nowrap rounded-b border border-cm-purplelight bg-[#2a0d52] ${align === 'right' ? 'right-0' : 'left-0'}`}
        >
          {options.map((o) => (
            <button
              key={o.key}
              onClick={() => {
                onChange?.(o.key)
                setOpen(false)
              }}
              className={`block w-full px-3 py-0.5 text-left hover:bg-cm-purple ${o.key === value ? 'text-cm-yellow' : 'text-white'}`}
            >
              {o.label}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

/** Texto clicável que abre outra tela (clube, jogador...). */
export function Link({ children, onClick, className = '' }: { children: ReactNode; onClick: () => void; className?: string }) {
  return (
    <span
      role="link"
      onClick={(e) => {
        e.stopPropagation()
        onClick()
      }}
      className={`cursor-pointer hover:underline ${className}`}
    >
      {children}
    </span>
  )
}
