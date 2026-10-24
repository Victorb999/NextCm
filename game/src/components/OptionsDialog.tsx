import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { BACKGROUND_PHOTOS } from '../backgrounds'
import {
  CURRENCIES,
  DEFAULT_SETTINGS,
  FONTS,
  MATCH_SPEEDS,
  RESOLUTIONS,
  SIDE_PRESETS,
  TEXT_FONTS,
  WAGES,
  applyResolution,
  applyScreenMode,
  getSettings,
  loadBackgroundImage,
  saveSettings,
  type Settings,
} from '../settings'

const VERSION = '0.2 (Fase 2)'

function Row({ label, children, disabled }: { label: string; children: ReactNode; disabled?: boolean }) {
  return (
    <div className={`flex min-h-[26px] items-center gap-3 ${disabled ? 'opacity-50' : ''}`}>
      <span className="w-48 shrink-0 font-title text-[19px] leading-none text-white">{label}</span>
      <div className="flex flex-1 flex-wrap items-center gap-2">{children}</div>
    </div>
  )
}

function Option({ selected, onClick, disabled, children }: { selected: boolean; onClick?: () => void; disabled?: boolean; children: ReactNode }) {
  return (
    <button aria-pressed={selected} disabled={disabled} onClick={onClick} className="cm-option min-w-[78px] py-0.5 disabled:cursor-default">
      {children}
    </button>
  )
}

/** Valor com a setinha do CM: clique avança, clique direito volta. */
function Cycle<T extends string>({
  options,
  value,
  onChange,
  disabled,
}: {
  options: { key: T; label: string }[]
  value: T
  onChange?: (v: T) => void
  disabled?: boolean
}) {
  const i = Math.max(0, options.findIndex((o) => o.key === value))
  const step = (d: number) => onChange?.(options[(i + d + options.length) % options.length].key)
  return (
    <div className="flex w-72 items-center justify-between">
      <span className="font-bold text-cm-yellow">{options[i].label}</span>
      <button
        disabled={disabled}
        onClick={() => step(1)}
        onContextMenu={(e) => {
          e.preventDefault()
          step(-1)
        }}
        className="flex h-5 w-5 items-center justify-center bg-black text-[10px] text-white"
        aria-label="Mudar"
      >
        ◀
      </button>
    </div>
  )
}

/** Diálogo "Definições do Jogo" (botão Opções Jogo). Só grava ao carregar em Ok. */
export default function OptionsDialog({ onClose }: { onClose: () => void }) {
  const [s, setS] = useState<Settings>(getSettings)
  const [error, setError] = useState<string | null>(null)
  const file = useRef<HTMLInputElement>(null)
  const set = (patch: Partial<Settings>) => setS((old) => ({ ...old, ...patch }))

  const ok = () => {
    const old = getSettings()
    const screenChanged = s.screen !== old.screen
    const sizeChanged = screenChanged || s.resolution !== old.resolution
    if (!saveSettings(s)) {
      setError('Não foi possível gravar (imagem grande demais?). Escolha outra imagem.')
      return
    }
    // Primeiro sai/entra do ecrã completo, depois o tamanho da janela
    if (sizeChanged) void (screenChanged ? applyScreenMode(s.screen) : Promise.resolve()).then(() => applyResolution(s.resolution, s.screen))
    onClose()
  }

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopImmediatePropagation()
        onClose()
      }
      if (e.key === 'Enter' && !(document.activeElement instanceof HTMLInputElement)) ok()
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  })

  const pickImage = async (f: File | undefined) => {
    if (!f) return
    try {
      set({ background: 'image', backgroundImage: await loadBackgroundImage(f) })
      setError(null)
    } catch (e) {
      setError(String(e))
    }
  }

  const customSide = !SIDE_PRESETS.some((p) => p.colour === s.sideColour)

  return (
    <div className="absolute inset-0 z-40 flex items-center justify-center bg-black/50 p-4">
      <div className="flex max-h-full w-[660px] max-w-full flex-col border-2 border-[#1d4a20] bg-[#0b3a10] p-2 shadow-[4px_4px_0_rgba(0,0,0,0.6)]">
        <div className="flex h-12 shrink-0 items-center justify-center border-2 border-[#333] bg-black font-title text-2xl text-white">
          Definições do Jogo
        </div>
        <div className="flex shrink-0 items-end justify-between px-2 py-1.5 text-[12px] text-white">
          <div className="leading-tight">
            Versão
            <br />
            {VERSION}
          </div>
          <button className="cm-purple h-6 w-36" onClick={() => setS(DEFAULT_SETTINGS)} title="Voltar às definições de origem">
            Anular
          </button>
        </div>
        <div
          className="min-h-0 flex-1 space-y-1.5 overflow-auto border-2 border-[#1d3a1f] px-3 py-2"
          style={{ background: 'repeating-linear-gradient(90deg, #0f5a16 0 70px, #126a1a 70px 140px)' }}
        >
          <Row label="Língua">
            <Cycle options={[{ key: 'pt', label: 'Português' }]} value="pt" disabled />
          </Row>
          <Row label="Moeda">
            <Cycle options={CURRENCIES} value={s.currency} onChange={(currency) => set({ currency })} />
          </Row>
          <Row label="Letra dos Títulos">
            <Cycle options={FONTS} value={s.font} onChange={(font) => set({ font })} />
          </Row>
          <Row label="Letra do Texto">
            <Cycle options={TEXT_FONTS} value={s.textFont} onChange={(textFont) => set({ textFont })} />
          </Row>
          <Row label="Remunerações">
            {WAGES.map((w) => (
              <Option key={w.key} selected={s.wages === w.key} onClick={() => set({ wages: w.key })}>
                {w.label}
              </Option>
            ))}
          </Row>
          <Row label="Velocidade Comentário">
            <Cycle options={MATCH_SPEEDS} value={s.matchSpeed} onChange={(matchSpeed) => set({ matchSpeed })} />
          </Row>
          <Row label="Modo Ecrã">
            <Option selected={s.screen === 'fullscreen'} onClick={() => set({ screen: 'fullscreen' })}>
              Completo
            </Option>
            <Option selected={s.screen === 'window'} onClick={() => set({ screen: 'window' })}>
              Em Janela
            </Option>
          </Row>
          <Row label="Resolução" disabled={s.screen === 'fullscreen'}>
            <Cycle
              options={RESOLUTIONS}
              value={s.resolution}
              onChange={(resolution) => set({ resolution })}
              disabled={s.screen === 'fullscreen'}
            />
          </Row>
          <Row label="Coluna Lateral">
            {SIDE_PRESETS.map((p) => (
              <button
                key={p.colour}
                title={p.label}
                aria-pressed={s.sideColour === p.colour}
                onClick={() => set({ sideColour: p.colour })}
                className={`h-6 w-9 border-2 ${s.sideColour === p.colour ? 'border-cm-yellow' : 'border-black'}`}
                style={{ background: p.colour }}
              />
            ))}
            <label
              title="Escolher outra cor"
              className={`flex h-6 cursor-pointer items-center gap-1 border-2 px-1 text-[12px] ${customSide ? 'border-cm-yellow text-cm-yellow' : 'border-black text-cm-green'}`}
            >
              <input
                type="color"
                value={s.sideColour}
                onChange={(e) => set({ sideColour: e.target.value })}
                className="h-4 w-6 cursor-pointer border-0 bg-transparent p-0"
              />
              Outra
            </label>
          </Row>
          <Row label="Fundo">
            <Option selected={s.background === 'default'} onClick={() => set({ background: 'default' })}>
              Padrão
            </Option>
            <Option selected={s.background === 'plain'} onClick={() => set({ background: 'plain' })}>
              Liso
            </Option>
            <Option
              selected={s.background === 'photo'}
              onClick={() => set({ background: 'photo', backgroundPhoto: s.backgroundPhoto ?? BACKGROUND_PHOTOS[0].key })}
            >
              Foto
            </Option>
            <Option
              selected={s.background === 'image'}
              onClick={() => (s.backgroundImage ? set({ background: 'image' }) : file.current?.click())}
            >
              Imagem
            </Option>
            <button className="cm-purple h-6" onClick={() => file.current?.click()}>
              Escolher…
            </button>
            <input
              ref={file}
              type="file"
              accept="image/*"
              className="hidden"
              onChange={(e) => {
                void pickImage(e.target.files?.[0])
                e.target.value = ''
              }}
            />
            {s.backgroundImage && (
              <span
                className="h-6 w-10 border border-black bg-cover bg-center"
                style={{ backgroundImage: `url("${s.backgroundImage}")` }}
                title="Imagem escolhida"
              />
            )}
          </Row>
          {s.background === 'photo' &&
            (['Estádios', 'Momentos'] as const).map((group) => (
              <Row key={group} label={group}>
                {BACKGROUND_PHOTOS.filter((p) => p.group === group).map((p) => (
                  <button
                    key={p.key}
                    title={p.label}
                    aria-pressed={s.backgroundPhoto === p.key}
                    onClick={() => set({ backgroundPhoto: p.key })}
                    className={`h-12 w-20 border-2 bg-cover bg-center ${s.backgroundPhoto === p.key ? 'border-cm-yellow' : 'border-black'}`}
                    style={{ backgroundImage: `url("${p.url}")` }}
                  />
                ))}
              </Row>
            ))}
          <Row label="Escurecer Fundo" disabled={s.background !== 'image' && s.background !== 'photo'}>
            {[0, 0.25, 0.5, 0.75].map((d) => (
              <Option
                key={d}
                selected={s.backgroundDim === d}
                disabled={s.background !== 'image' && s.background !== 'photo'}
                onClick={() => set({ backgroundDim: d })}
              >
                {d * 100}%
              </Option>
            ))}
          </Row>
          <Row label="Som" disabled>
            <Option selected={false} disabled>
              Sim
            </Option>
            <Option selected disabled>
              Não
            </Option>
          </Row>
          <Row label="Auto Gravação" disabled>
            <Cycle options={[{ key: 'never', label: 'Nunca (Fase 3)' }]} value="never" disabled />
          </Row>
          {error && <div className="bg-red-900 px-2 py-1 text-white">{error}</div>}
        </div>
        <div className="mt-2 flex shrink-0 gap-1">
          <button className="cm-purple h-9 flex-[2] font-title text-xl" onClick={onClose}>
            Cancelar
          </button>
          <button className="cm-purple h-9 flex-1 font-title text-xl" onClick={ok}>
            Ok
          </button>
        </div>
      </div>
    </div>
  )
}
