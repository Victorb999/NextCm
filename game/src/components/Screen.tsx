import { ChevronDown } from 'lucide-react'
import { useState } from 'react'
import type { ReactNode } from 'react'

/** Faixa de título das telas do CM: caixa colorida com o nome grande centralizado. */
export function TitleBand({ title, fg, bg }: { title: ReactNode; fg: string; bg: string }) {
  return (
    <div className="shrink-0 p-1.5 pb-1">
      <div
        className="flex h-14 items-center justify-center border-2 border-black font-title text-[30px] font-bold tracking-tight"
        style={{ color: fg, background: bg, textShadow: '1px 1px 0 rgba(0,0,0,0.35)' }}
      >
        {title}
      </div>
    </div>
  )
}

export type Tab = { key: string; label: string; disabled?: boolean }

/** Linha de abas do CM: escuras, a selecionada com borda amarela. */
export function Tabs({ tabs, active, onChange }: { tabs: Tab[]; active: string; onChange: (key: string) => void }) {
  return (
    <div className="flex shrink-0 gap-0.5 px-1.5" role="tablist">
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
    <div className="flex h-11 shrink-0 gap-0.5 p-1.5 pt-1">
      <button className="cm-purple flex-[2] font-title text-xl" disabled={!onLeft} onClick={onLeft}>
        {left}
      </button>
      <button className="cm-purple flex-1 font-title text-xl" disabled={!onRight} onClick={onRight}>
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
}: {
  label: string
  options: { key: T; label: string }[]
  value?: T
  onChange?: (key: T) => void
  disabled?: boolean
}) {
  const [open, setOpen] = useState(false)
  return (
    <div className="relative" onMouseLeave={() => setOpen(false)}>
      <button
        className="cm-purple flex h-6 w-40 items-center justify-center gap-1"
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
      >
        {label} <ChevronDown size={14} />
      </button>
      {open && (
        <div className="absolute left-0 top-6 z-30 w-40 border border-cm-purplelight bg-[#2a0d52]">
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
