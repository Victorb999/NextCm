import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { BACKGROUND_PHOTOS } from '../backgrounds'
import {
  AUTOSAVES,
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

type Section = 'general' | 'screen' | 'look'
const SECTIONS: { key: Section; label: string }[] = [
  { key: 'general', label: 'Geral' },
  { key: 'screen', label: 'Ecrã' },
  { key: 'look', label: 'Aparência' },
]

function Row({ label, children, disabled }: { label: string; children: ReactNode; disabled?: boolean }) {
  return (
    <div className={`flex min-h-[40px] items-center gap-3 px-3 py-1 ${disabled ? 'opacity-50' : ''}`}>
      <span className="w-48 shrink-0 text-[15px] font-semibold text-white">{label}</span>
      <div className="flex flex-1 flex-wrap items-center gap-2">{children}</div>
    </div>
  )
}

function Option({ selected, onClick, disabled, children }: { selected: boolean; onClick?: () => void; disabled?: boolean; children: ReactNode }) {
  return (
    <button
      aria-pressed={selected}
      disabled={disabled}
      onClick={onClick}
      className={`h-8 min-w-[78px] rounded-[4px] border px-3 text-[13px] disabled:cursor-default ${
        selected ? 'border-cm-yellow bg-[#1d4d3a] font-bold text-cm-yellow' : 'border-[#2f6b52] text-[#d6efe4] enabled:hover:bg-[#1d4d3a]/60'
      }`}
    >
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
    <div className="flex h-8 w-72 items-center justify-between rounded-[4px] border border-[#2f6b52] bg-[#0b1a14] pl-3 pr-1">
      <span className="font-bold text-cm-yellow">{options[i].label}</span>
      <button
        disabled={disabled}
        onClick={() => step(1)}
        onContextMenu={(e) => {
          e.preventDefault()
          step(-1)
        }}
        className="flex h-6 w-6 items-center justify-center rounded-[3px] bg-black/50 text-[10px] text-white hover:bg-black"
        aria-label="Mudar"
      >
        ◀
      </button>
    </div>
  )
}

/** Diálogo "Definições do Jogo" (botão Opções Jogo), em abas Geral / Ecrã / Aparência. Só grava ao carregar em Ok. */
export default function OptionsDialog({ onClose }: { onClose: () => void }) {
  const [s, setS] = useState<Settings>(getSettings)
  const [error, setError] = useState<string | null>(null)
  const [section, setSection] = useState<Section>('general')
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
      <div className="flex h-[min(620px,100%)] w-[720px] max-w-full flex-col overflow-hidden rounded-lg border-2 border-[#10b981] bg-[#0f241b] shadow-[0_12px_32px_rgba(0,0,0,0.7)]">
        <div className="flex h-12 shrink-0 items-center gap-2 border-b border-[#2f6b52] px-4">
          <span className="size-2 rounded-full bg-[#4edea3]" />
          <span className="font-mono font-bold uppercase tracking-[0.12em] text-white">Definições do Jogo</span>
          <span className="ml-auto text-[12px] text-[#86869a]">Versão {VERSION}</span>
          <button className="cm-purple h-7 w-28" onClick={() => setS(DEFAULT_SETTINGS)} title="Voltar às definições de origem">
            Anular
          </button>
        </div>
        <div className="flex shrink-0 gap-1.5 px-4 pt-3" role="tablist">
          {SECTIONS.map((t) => (
            <button key={t.key} role="tab" aria-selected={section === t.key} onClick={() => setSection(t.key)} className="cm-tab h-8">
              {t.label}
            </button>
          ))}
        </div>
        <div className="cm-zebra m-4 mt-3 min-h-0 flex-1 overflow-auto rounded-md border border-[#2f6b52]">
          {section === 'general' && (
            <>
              <Row label="Língua">
                <Cycle options={[{ key: 'pt', label: 'Português' }]} value="pt" disabled />
              </Row>
              <Row label="Moeda">
                <Cycle options={CURRENCIES} value={s.currency} onChange={(currency) => set({ currency })} />
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
              <Row label="Auto Gravação">
                <Cycle options={AUTOSAVES} value={s.autosave} onChange={(autosave) => set({ autosave })} />
              </Row>
              <Row label="Som" disabled>
                <Option selected={false} disabled>
                  Sim
                </Option>
                <Option selected disabled>
                  Não
                </Option>
              </Row>
            </>
          )}
          {section === 'screen' && (
            <>
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
            </>
          )}
          {section === 'look' && (
            <>
              <Row label="Letra dos Títulos">
                <Cycle options={FONTS} value={s.font} onChange={(font) => set({ font })} />
              </Row>
              <Row label="Letra do Texto">
                <Cycle options={TEXT_FONTS} value={s.textFont} onChange={(textFont) => set({ textFont })} />
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
            </>
          )}
        </div>
        {error && <div className="mx-4 -mt-1 mb-3 rounded bg-red-900 px-2 py-1 text-white">{error}</div>}
        <div className="flex shrink-0 justify-end gap-2 border-t border-[#2f6b52] p-3">
          <button className="h-9 w-32 rounded-[4px] border border-white/20 bg-black/30 font-semibold text-white hover:bg-black/50" onClick={onClose}>
            Cancelar
          </button>
          <button className="h-9 w-32 rounded-[4px] bg-[#10b981] font-bold text-[#06281b] hover:bg-[#4edea3]" onClick={ok}>
            Ok
          </button>
        </div>
      </div>
    </div>
  )
}
