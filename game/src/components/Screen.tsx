import { ChevronDown } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import type { ClubRef } from '../bindings/ClubRef'
import { initials } from '../format'

/** As cores de um clube, com o cinzento de sempre quando o patch não as tem. */
function clubColours(club: Pick<ClubRef, 'fg' | 'bg'>) {
  return club.fg && club.bg ? { fg: club.fg, bg: club.bg } : { fg: '#ffffff', bg: '#3a3a3a' }
}

/** Cabeçalho de todas as telas: emblema do clube (ou ícone), etiqueta, título grande e algo à direita. */
export function ScreenHeader({
  title,
  icon,
  club,
  emblem,
  eyebrow,
  right,
}: {
  title: ReactNode
  icon?: ReactNode
  /** O clube da tela: o emblema com as iniciais nas cores dele. */
  club?: Pick<ClubRef, 'shortName' | 'fg' | 'bg'> | null
  /** Um emblema à mão (sem clube). */
  emblem?: { text: string; fg: string; bg: string }
  /** Linha pequena por cima do título (nome do clube ou do jogador, etiquetas, época). */
  eyebrow?: ReactNode
  right?: ReactNode
}) {
  const badge = emblem ?? (club ? { text: initials(club.shortName), ...clubColours(club) } : undefined)
  return (
    <div className="m-2 flex shrink-0 items-center gap-3 rounded-md bg-[#1a1b21] px-3 py-1.5">
      {badge && (
        <span
          className="flex size-9 shrink-0 items-center justify-center rounded font-title text-[15px] font-bold"
          style={{ color: badge.fg, background: badge.bg }}
        >
          {badge.text}
        </span>
      )}
      {icon && <span className="shrink-0 text-[#c8c8d0]">{icon}</span>}
      <div className="min-w-0">
        {eyebrow && <div className="flex items-center gap-2 text-[11px] font-bold font-mono uppercase tracking-[0.12em] text-[#c8c8d0]">{eyebrow}</div>}
        <div className="truncate font-title text-[20px] font-bold leading-tight text-white">{title}</div>
      </div>
      <div className="ml-auto flex shrink-0 items-center gap-2">{right}</div>
    </div>
  )
}

/** Painel arredondado com título laranja em mono (o padrão das telas novas); `chip` é a etiqueta verde.
 * Sem `overflow-hidden`: os menus do título (Vêr) abrem por cima do resto. */
export function Panel({
  title,
  chip,
  right,
  children,
  className = '',
}: {
  title?: ReactNode
  chip?: ReactNode
  right?: ReactNode
  children: ReactNode
  className?: string
}) {
  return (
    <div className={`flex min-h-0 flex-col rounded-md bg-[#1a1b21] ${className}`}>
      {title && <PanelTitle title={title} chip={chip} right={right} />}
      {children}
    </div>
  )
}

/** A barra de título de um painel, para painéis montados de outra forma. */
export function PanelTitle({ title, chip, right }: { title: ReactNode; chip?: ReactNode; right?: ReactNode }) {
  return (
    <div className="flex h-8 shrink-0 items-center gap-2 border-b border-cm-line px-3 text-[12px] font-bold font-mono uppercase tracking-[0.12em] text-cm-orange">
      {title}
      {chip && <span className="rounded bg-[#0f3a2a] px-1.5 tracking-wider text-[#4edea3]">{chip}</span>}
      <span className="ml-auto flex min-w-0 items-center gap-1.5 truncate normal-case tracking-normal">{right}</span>
    </div>
  )
}

/** Faixa vermelha de erro. */
export function ErrorBox({ children, className = '' }: { children: ReactNode; className?: string }) {
  if (!children) return null
  return <div className={`rounded bg-red-900 px-2 py-1 text-white ${className}`}>{children}</div>
}

/** "Sem jogos marcados.", "Sem notícias."...: lista vazia, ao centro em magenta. */
export function Empty({ children, className = 'p-6' }: { children: ReactNode; className?: string }) {
  return <div className={`text-center text-cm-magenta ${className}`}>{children}</div>
}

export type Tab = { key: string; label: string; disabled?: boolean }

/** Linha de abas do CM: escuras, a selecionada com borda amarela. */
export function Tabs({ tabs, active, onChange }: { tabs: Tab[]; active: string; onChange: (key: string) => void }) {
  return (
    <div className="flex shrink-0 gap-1.5 px-2" role="tablist">
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
  /** Uma tela montada mas escondida (a Pesquisa): não importa, a barra fica com ela. */
  active?: boolean
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

/** Uma opção do menu, ou um traço entre grupos. */
export type MenuOption<T extends string> = { key: T; label: string } | 'separator'

/** Botão roxo com menu ("Vêr ▼", "Critério ▼", "Acção ▼"), como os do CM: abre e fecha no clique
 * (e fecha com um clique fora ou com Esc); a lista no verde dos menus, com zebra. */
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
  options: MenuOption<T>[]
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
  const box = useRef<HTMLDivElement>(null)
  // Com a lista aberta, o Escape só a fecha (não volta à tela anterior); um clique fora fecha-a
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      e.stopImmediatePropagation()
      setOpen(false)
    }
    const onDown = (e: MouseEvent) => {
      if (!box.current?.contains(e.target as Node)) setOpen(false)
    }
    window.addEventListener('keydown', onKey, true)
    window.addEventListener('mousedown', onDown)
    return () => {
      window.removeEventListener('keydown', onKey, true)
      window.removeEventListener('mousedown', onDown)
    }
  }, [open])
  return (
    <div ref={box} className="relative">
      <button
        className={buttonClass ?? 'cm-purple flex h-6 w-40 items-center justify-center gap-1'}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
        aria-label={trigger ? label : undefined}
        aria-expanded={open}
      >
        {trigger ?? (
          <>
            {label} <ChevronDown size={14} />
          </>
        )}
      </button>
      {open && (
        <div
          className={`cm-zebra absolute top-full z-30 mt-1 min-w-44 overflow-hidden whitespace-nowrap rounded-md border border-[#10b981]/60 bg-[#0f241b] py-1 text-[14px] shadow-[0_8px_24px_rgba(0,0,0,0.65)] ${
            align === 'right' ? 'right-0' : 'left-0'
          }`}
        >
          {options.map((o, i) =>
            o === 'separator' ? (
              <div key={`s${i}`} className="mx-2 my-1 border-t border-[#2f6b52]" />
            ) : (
              <button
                key={o.key}
                onClick={() => {
                  onChange?.(o.key)
                  setOpen(false)
                }}
                className={`flex h-6 w-full items-center gap-2 px-3 text-left hover:!bg-[#1d4d3a] hover:text-cm-yellow ${
                  o.key === value ? 'font-bold text-cm-yellow' : 'text-[#e3e1e9]'
                }`}
              >
                <span className="w-3 shrink-0 text-[11px]">{o.key === value ? '✓' : ''}</span>
                {o.label}
              </button>
            ),
          )}
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
