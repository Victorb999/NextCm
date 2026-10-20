import { useEffect, useState } from 'react'
import { api } from '../api'
import type { ClubTactics } from '../bindings/ClubTactics'
import type { LiveView } from '../bindings/LiveView'
import type { Selection } from '../bindings/Selection'
import { selectionOf, swap, useDragSwap, type Spot } from '../selection'
import InstructionsDialog from './InstructionsDialog'
import { BottomBar, PurpleMenu, Tabs, TitleBand } from './Screen'
import { Pitch, Row } from './TacticsBoard'
import { TACTICS_MENU, moveOnPitch, shortRole, surname, useRunArrow } from '../tactics'

/**
 * "Tácticas <clube>" a meio do jogo: a mesma tela de Tácticas (arrastar na lista ou no
 * campo, formações e Instruções à Equipa no menu Tácticas, posições livres, setas com o
 * botão direito), só com quem está no jogo. Trocar um titular com um suplente é uma
 * substituição. Nada vai para o jogo até ao Ok; o Cancelar esquece tudo.
 */
export default function LiveTactics({
  view,
  side,
  onClose,
  onApplied,
}: {
  view: LiveView
  side: 0 | 1
  onClose: () => void
  onApplied: (v: LiveView) => void
}) {
  const [t, setT] = useState<ClubTactics | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [instructions, setInstructions] = useState(false)
  const [busy, setBusy] = useState(false)
  // Como estava ao abrir: quem estava em campo (para contar as substituições) e a escolha
  const [original, setOriginal] = useState<{ on: number[]; sel: Selection } | null>(null)
  const team = side === 0 ? view.home : view.away
  const editable = !!t?.editable

  useEffect(() => {
    api.liveTactics(side).then(
      (tt) => {
        setOriginal({ on: tt.starters.flatMap((s) => (s.player ? [s.player.id] : [])), sel: selectionOf(tt) })
        setT(tt)
      },
      (e) => setError(String(e)),
    )
  }, [side])

  /** Mostra como fica (funções e notas vêm do core), sem mexer no jogo. */
  const preview = async (sel: Selection) => {
    try {
      setT(await api.livePreviewTactics(sel))
      setError(null)
    } catch (e) {
      setError(String(e))
    }
  }

  const arrow = useRunArrow(t, (sel) => void preview(sel))
  const drag = useDragSwap({
    enabled: editable,
    onDrop: (from: Spot, to: Spot) => {
      if (t) void preview(swap(selectionOf(t), from, to))
    },
    onDropPitch: (from, x, y) => {
      if (!t || from.kind !== 'starter') return
      const sel = moveOnPitch(t, from.index, x, y)
      if (sel) void preview(sel)
    },
  })

  const entering = t && original ? t.starters.filter((s) => s.player && !original.on.includes(s.player.id)).length : 0
  const subsLeft = view.minute === 0 ? team.subsLeft : team.subsLeft - entering

  const ok = async () => {
    if (!t || busy) return
    const sel = selectionOf(t)
    if (!editable || JSON.stringify(sel) === JSON.stringify(original?.sel)) return onClose()
    setBusy(true)
    try {
      onApplied(await api.liveSetTactics(sel))
      onClose()
    } catch (e) {
      setError(String(e))
      setBusy(false)
    }
  }

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (instructions) return
      if (e.key === 'Escape') {
        e.stopImmediatePropagation()
        onClose()
      } else if (e.key === 'Enter') {
        e.stopImmediatePropagation()
        e.preventDefault()
        void ok()
      }
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  })

  const fg = team.club.fg ?? '#ffffff'
  const bg = team.club.bg ?? '#3a3a3a'
  const label = (s: { player: { listName: string } | null; role: string }) => (s.player ? surname(s.player.listName) : s.role)

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <TitleBand title={`Tácticas ${t?.clubName ?? team.club.name}`} fg={fg} bg={bg} />
      <div className="flex shrink-0 items-center gap-1 px-1.5">
        <PurpleMenu
          label="Tácticas"
          options={TACTICS_MENU}
          value={t && !t.custom ? String(t.formationIndex) : undefined}
          disabled={!editable}
          onChange={(k) => {
            if (k === 'instructions') setInstructions(true)
            else if (t) void preview({ ...selectionOf(t), formation: Number(k), positions: [] })
          }}
        />
        <PurpleMenu label="Vêr" options={[]} disabled />
        <div className="flex-1" />
        {editable && (
          <span className="px-2 text-[12px] text-cm-yellow">
            {view.minute === 0 ? 'Antes do apito as trocas não contam' : `Substituições restantes: ${subsLeft}`}
          </span>
        )}
        <button className="cm-purple h-6 w-40" disabled>
          Último Jogo
        </button>
        <PurpleMenu
          label="Editar"
          align="right"
          options={[{ key: 'reset', label: 'Repor as posições da táctica' }]}
          disabled={!editable}
          onChange={() => {
            if (t) void preview({ ...selectionOf(t), positions: [] })
          }}
        />
      </div>
      {error && <div className="mx-1.5 mt-1.5 bg-red-900 px-2 py-1 text-white">{error}</div>}
      {t ? (
        <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 gap-1.5">
          <div className="flex min-h-0 w-[50%] flex-col">
            <div className="flex h-10 shrink-0 items-center justify-center bg-black/70 font-title text-[20px] font-bold text-cm-yellow">
              {t.formation}
              {t.custom && '*'}
            </div>
            <div className="min-h-0 flex-1 overflow-auto bg-black/50 py-1">
              {t.starters.map((s, i) => {
                const spot: Spot = { kind: 'starter', index: i }
                return s.player ? (
                  <Row key={`s${i}`} p={s.player} kind="starter" place={shortRole(s.role)} bind={drag.bind(spot, label(s))} />
                ) : (
                  <div key={`s${i}`} {...drag.bind(spot, s.role)} className="flex h-[19px] items-center gap-2 px-1 text-[13px]">
                    <span className="w-7 shrink-0" />
                    <span className="cm-cell w-9 shrink-0 text-center text-[11px] text-cm-yellow">{shortRole(s.role)}</span>
                    <span className="italic text-[#8a8a8a]">— sem jogador (expulso) —</span>
                  </div>
                )
              })}
              <div className="my-0.5 border-t border-cm-line" />
              {t.bench.map((p, i) => {
                // Quem estava em campo e foi para o banco nesta tela sai de vez no Ok (antes do apito volta ao banco)
                const leaving = view.minute > 0 && !!original?.on.includes(p.id)
                return leaving ? (
                  <div key={`b${p.id}`} className="relative">
                    <Row p={p} kind="reserve" place="Sai" bind={drag.bind({ kind: 'bench', index: i }, surname(p.listName))} />
                    <span className="pointer-events-none absolute right-[4.5rem] top-0 text-[11px] font-bold leading-[19px] text-[#ff6060]">
                      sai no Ok
                    </span>
                  </div>
                ) : (
                  <Row key={`b${p.id}`} p={p} kind="bench" place={`SB${i + 1}`} bind={drag.bind({ kind: 'bench', index: i }, surname(p.listName))} />
                )
              })}
              {/* Quem já saiu não pode voltar: só aparece, sem se poder arrastar */}
              {team.out.length > 0 && (
                <>
                  <div className="my-0.5 border-t border-cm-line" />
                  {team.out.map((p) => (
                    <div key={`o${p.id}`} className="flex h-[19px] items-center gap-2 px-1 text-[13.5px] text-[#6a6a6a]">
                      <span className="cm-cell w-7 shrink-0 text-center">{p.squadNumber ?? ''}</span>
                      <span className="cm-cell w-9 shrink-0 text-center text-[11px]">{p.red ? 'Exp' : 'Saiu'}</span>
                      <span className="flex-1 truncate">{p.listName.split(', ').reverse().join(' ')}</span>
                      <span className="shrink-0 text-[12px]">{p.red ? `expulso ${p.offMinute}'` : `saiu ${p.offMinute}'`}</span>
                    </div>
                  ))}
                </>
              )}
            </div>
          </div>
          <div className="flex min-h-0 flex-1 flex-col">
            <Tabs
              tabs={[
                { key: 'overview', label: 'Vista Geral' },
                { key: 'with', label: 'Com Bola', disabled: true },
                { key: 'without', label: 'Sem Bola', disabled: true },
              ]}
              active="overview"
              onChange={() => {}}
            />
            <div className="mt-1 flex min-h-0 flex-1 flex-col">
              <Pitch t={t} bind={drag.bind} run={arrow.run} onRunStart={arrow.start} />
            </div>
            <div className="shrink-0 pt-1 text-center text-[12px] text-[#aab3c4]">
              {editable
                ? 'Arraste para trocar (titular com suplente = substituição) ou para um lugar vazio · botão direito e arrastar faz a seta'
                : 'Só pode mexer na sua equipa.'}
            </div>
          </div>
        </div>
      ) : (
        <div className="flex-1" />
      )}
      {drag.ghost}
      {instructions && t && (
        <InstructionsDialog
          initial={t.instructions}
          onCancel={() => setInstructions(false)}
          onOk={(i) => {
            setInstructions(false)
            void preview({ ...selectionOf(t), instructions: i })
          }}
        />
      )}
      <BottomBar left="Cancelar" right="Ok" onLeft={onClose} onRight={busy ? undefined : () => void ok()} />
    </div>
  )
}
