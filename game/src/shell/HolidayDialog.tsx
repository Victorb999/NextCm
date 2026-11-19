import { useEffect, useRef, useState } from 'react'
import { api } from '../api'
import type { HolidayPlan } from '../bindings/HolidayPlan'
import type { HolidaySales } from '../bindings/HolidaySales'
import type { HolidayView } from '../bindings/HolidayView'
import { DialogFrame, OptionGroup, OptionLine } from '../components/Dialog'
import { Empty } from '../components/Screen'
import { cmDate } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'

// Ir de Férias: as Configurações do CM 01/02 (com as do FM08: voltar por um evento e a data
// de volta) e, depois do Sim, o "De Férias" com os dias a passar e o Voltar das Férias.

const DURATIONS: [number, string][] = [
  [7, '1 Semana'],
  [14, '2 Semanas'],
  [30, '1 Mês'],
  [90, '3 Meses'],
]

const SALES: [HolidaySales, string][] = [
  ['accept', 'Aceitar todas as propostas por jogadores'],
  ['asking', 'Rejeitar todas as propostas abaixo do valor exigido'],
  ['reject', 'Rejeitar todas as propostas por jogadores'],
]

type Back = 'days' | 'date' | 'never'

/** Por cima de tudo (a coluna e a barra do topo também): nas férias não se mexe no jogo. */
function Cover({ children }: { children: React.ReactNode }) {
  return <div className="fixed inset-0 z-[80] flex items-center justify-center bg-black/60 p-4">{children}</div>
}

export default function HolidayDialog({ onClose, notify }: { onClose: () => void; notify: (msg: string) => void }) {
  const game = useGame()
  // Reaberto com as férias a meio (o app recarregado): vai direto ao "De Férias"
  const away = game.info?.holiday ?? false
  const [view, setView] = useState<HolidayView | null>(null)
  useEffect(() => {
    if (away) api.holidayView().then(setView, (e) => notify(String(e)))
  }, [away, notify])
  if (away || view) return view ? <Away first={view} onClose={onClose} notify={notify} /> : null
  return <Options onCancel={onClose} onStart={setView} notify={notify} />
}

function Options({
  onCancel,
  onStart,
  notify,
}: {
  onCancel: () => void
  onStart: (v: HolidayView) => void
  notify: (msg: string) => void
}) {
  const [plan, setPlan] = useState<HolidayPlan>({
    applyJobs: false,
    returnOnJob: true,
    returnOnWarning: true,
    sales: 'asking',
    listedOnly: false,
    useTactics: true,
    useTeam: false,
    days: 14,
    until: null,
  })
  const [back, setBack] = useState<Back>('days')
  const [days, setDays] = useState(14)
  const [until, setUntil] = useState('')
  const [busy, setBusy] = useState(false)
  const set = (p: Partial<HolidayPlan>) => setPlan((cur) => ({ ...cur, ...p }))

  const start = async () => {
    setBusy(true)
    try {
      const chosen = { ...plan, days: back === 'days' ? days : null, until: back === 'date' ? until || null : null }
      if (back === 'date' && !chosen.until) throw new Error('Escolha o dia da volta.')
      onStart(await api.startHoliday(chosen))
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e))
      setBusy(false)
    }
  }

  return (
    <Cover>
      <DialogFrame
        title="Confirmar Por Favor"
        width={640}
        cancelLabel="Não"
        okLabel="Sim"
        onCancel={busy ? undefined : onCancel}
        onOk={busy ? undefined : () => void start()}
      >
        <div className="pb-2 text-center font-title text-[20px] text-white">Ir de férias?</div>
        <div className="pb-2 text-[13px] text-cm-dlgtext">
          Clique numa opção para ligar (branco) ou desligar (roxo). Nas férias o auxiliar escala o time e não contrata ninguém.
        </div>
        <OptionGroup title="Empregos">
          <OptionLine
            label="Se candidatar a vagas de técnico em clubes de maior reputação"
            on={plan.applyJobs}
            onToggle={() => set({ applyJobs: !plan.applyJobs })}
          />
          <OptionLine
            label="Voltar ao receber proposta de emprego"
            on={plan.returnOnJob}
            onToggle={() => set({ returnOnJob: !plan.returnOnJob })}
          />
          <OptionLine
            label="Voltar quando a Diretoria fizer um aviso"
            on={plan.returnOnWarning}
            onToggle={() => set({ returnOnWarning: !plan.returnOnWarning })}
          />
        </OptionGroup>
        <OptionGroup title="Propostas por jogadores">
          {SALES.map(([key, label]) => (
            <OptionLine key={key} radio label={label} on={plan.sales === key} onToggle={() => set({ sales: key })} />
          ))}
          <OptionLine
            label="Vender só jogadores da lista de transferências"
            on={plan.listedOnly}
            disabled={plan.sales === 'reject'}
            onToggle={() => set({ listedOnly: !plan.listedOnly })}
          />
        </OptionGroup>
        <OptionGroup title="Partidas">
          <OptionLine
            label="Usar as táticas de jogo atuais"
            on={plan.useTactics || plan.useTeam}
            disabled={plan.useTeam}
            onToggle={() => set({ useTactics: !plan.useTactics })}
          />
          <OptionLine
            label="Usar o time escolhido atualmente quando possível"
            on={plan.useTeam}
            onToggle={() => set({ useTeam: !plan.useTeam })}
          />
        </OptionGroup>
        <OptionGroup title="Volta" radio>
          <OptionLine radio label="Voltar depois de" on={back === 'days'} onToggle={() => setBack('days')}>
            {DURATIONS.map(([d, label]) => (
              <button
                key={d}
                aria-pressed={back === 'days' && days === d}
                className="cm-option py-0.5 text-[14px]"
                onClick={() => {
                  setBack('days')
                  setDays(d)
                }}
              >
                {label}
              </button>
            ))}
          </OptionLine>
          <OptionLine radio label="Voltar no dia" on={back === 'date'} onToggle={() => setBack('date')}>
            <input
              type="date"
              aria-label="Dia da volta"
              value={until}
              onChange={(e) => {
                setUntil(e.target.value)
                setBack('date')
              }}
              className="h-7 rounded border border-cm-dlgline bg-black px-2 text-[14px] text-cm-yellow outline-none [color-scheme:dark] focus:border-cm-yellow"
            />
          </OptionLine>
          <OptionLine radio label="Sem data (volta pelo botão ou por um aviso)" on={back === 'never'} onToggle={() => setBack('never')} />
        </OptionGroup>
      </DialogFrame>
    </Cover>
  )
}

/** "De Férias": os dias passam um a um até à volta, a um evento ou ao botão. */
function Away({ first, onClose, notify }: { first: HolidayView; onClose: () => void; notify: (msg: string) => void }) {
  const game = useGame()
  const nav = useNav()
  const [view, setView] = useState(first)
  const [leaving, setLeaving] = useState(false)
  // Um pedido de cada vez: o dia em curso acaba antes da volta (e o StrictMode não passa dois)
  const busy = useRef(false)

  useEffect(() => {
    if (busy.current) return
    if (leaving) {
      busy.current = true
      api.endHoliday().then(
        (info) => {
          game.update(info)
          nav.go({ kind: 'news', latest: true })
          onClose()
        },
        (e) => {
          busy.current = false
          notify(String(e))
        },
      )
      return
    }
    if (view.done) return
    busy.current = true
    api.holidayDay().then(
      (v) => {
        busy.current = false
        setView(v)
      },
      (e) => {
        busy.current = false
        notify(String(e))
        setLeaving(true)
      },
    )
  }, [view, leaving, game, nav, onClose, notify])

  const status = leaving
    ? 'Voltando…'
    : view.done
      ? view.stop
        ? 'As férias foram interrompidas.'
        : 'As férias acabaram.'
      : 'Os dias estão passando…'
  return (
    <Cover>
      <DialogFrame
        title="De Férias"
        width={620}
        footer={
          <button
            className="cm-purple h-9 flex-1 font-title text-xl enabled:text-cm-yellow"
            disabled={leaving}
            onClick={() => setLeaving(true)}
          >
            {view.done ? 'Voltar' : 'Voltar das Férias'}
          </button>
        }
      >
        <div className="grid grid-cols-3 gap-2 pb-2 text-center">
          <Fact label="Hoje" value={cmDate(view.date)} />
          <Fact label="Volta" value={view.until ? cmDate(view.until) : 'Sem data'} />
          <Fact label="Fora há" value={`${view.days} ${view.days === 1 ? 'dia' : 'dias'}`} />
        </div>
        <div aria-live="polite" className="pb-2 text-center font-title text-[17px] text-white">
          {status}
          {view.stop && <div className="font-bold text-cm-yellow">{view.stop}</div>}
        </div>
        <div className="font-title text-[18px] font-bold text-cm-yellow">Jogos</div>
        <div className="max-h-52 overflow-auto rounded bg-black/40">
          <div className="cm-zebra">
            {view.games.map((g) => (
              <div key={g.date + g.opponent} className="flex h-7 items-center gap-2 px-3 text-[14px]">
                <span className="w-20 shrink-0 text-cm-yellow">{cmDate(g.date)}</span>
                <span className="w-32 shrink-0 truncate text-cm-dlgtext">{g.competition}</span>
                <span className="min-w-0 flex-1 truncate font-semibold text-white">{g.opponent}</span>
                <span className="w-5 shrink-0 text-center text-cm-dlgtext">{g.venue}</span>
                <span className="w-10 shrink-0 text-right font-bold tabular-nums text-white">{g.score}</span>
                <span className={`w-5 shrink-0 text-center font-bold ${g.outcome === 'D' ? 'text-cm-orange' : 'text-cm-yellow'}`}>
                  {g.outcome}
                </span>
              </div>
            ))}
          </div>
          {view.games.length === 0 && <Empty className="p-3">O time ainda não jogou.</Empty>}
        </div>
        {view.notes.length > 0 && (
          <>
            <div className="pt-2 font-title text-[18px] font-bold text-cm-yellow">O auxiliar</div>
            <ul className="cm-zebra max-h-28 overflow-auto rounded bg-black/40 text-[14px] text-white">
              {view.notes.map((n, i) => (
                <li key={i} className="px-3 py-1">
                  {n}
                </li>
              ))}
            </ul>
          </>
        )}
      </DialogFrame>
    </Cover>
  )
}

function Fact({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded bg-black/40 py-1">
      <div className="text-[12px] uppercase text-cm-dlgtext">{label}</div>
      <div className="font-title text-[18px] font-bold text-white">{value}</div>
    </div>
  )
}
