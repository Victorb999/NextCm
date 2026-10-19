import { useEffect, useRef, useState } from 'react'
import { api } from '../api'
import type { ClubProfile } from '../bindings/ClubProfile'
import type { ClubTactics } from '../bindings/ClubTactics'
import type { Selection } from '../bindings/Selection'
import type { TacticPlayer } from '../bindings/TacticPlayer'
import InstructionsDialog from '../components/InstructionsDialog'
import { BottomBar, Link, PurpleMenu, Tabs, TitleBand } from '../components/Screen'
import { Pitch, Row, TACTICS_MENU, moveOnPitch, shortRole, surname, useRunArrow } from '../components/TacticsBoard'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'
import { BENCH, selectionOf, swap, useDragSwap, type Spot } from '../selection'

/**
 * Tela "Tácticas <clube>". No clube do treinador as mudanças valem na hora (arrastar troca,
 * clicar no número ou na casa põe/tira da equipa); Cancelar volta ao que estava ao abrir.
 */
export default function TacticsPage({ id }: { id: number }) {
  const nav = useNav()
  const game = useGame()
  useEscBack()
  const [t, setT] = useState<ClubTactics | null>(null)
  const [club, setClub] = useState<ClubProfile | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [instructions, setInstructions] = useState(false)
  // A escolha ao abrir a tela, para o Cancelar
  const original = useRef<Selection | null>(null)
  const changed = useRef(false)

  useEffect(() => {
    api.getClubTactics(id).then(
      (tt) => {
        original.current = selectionOf(tt)
        setT(tt)
      },
      (e) => setError(String(e)),
    )
    api.getClub(id).then(setClub, () => {})
  }, [id])

  const colours = club?.club.fg && club?.club.bg ? { fg: club.club.fg, bg: club.club.bg } : { fg: '#ffffff', bg: '#3a3a3a' }
  const name = club?.club.shortName ?? t?.clubName ?? ''
  const editable = !!t?.editable

  /** Grava a escolha já (a táctica do jogo muda na hora) e mostra o resultado. */
  const apply = async (sel: Selection) => {
    try {
      setT(await api.setTactics(sel))
      changed.current = true
      setError(null)
      const info = await api.gameInfo()
      if (info) game.update(info)
    } catch (e) {
      setError(String(e))
    }
  }

  const toggle = async (player: number) => {
    if (!t) return
    try {
      const next = await api.togglePlayer(selectionOf(t), player)
      await apply(selectionOf(next))
    } catch (e) {
      setError(String(e))
    }
  }

  // Seta de movimento a ser desenhada com o botão direito
  const arrow = useRunArrow(t, (sel) => void apply(sel))

  const drag = useDragSwap({
    enabled: editable,
    onDrop: (from, to) => {
      if (t) void apply(swap(selectionOf(t), from, to))
    },
    // Soltar um titular num lugar vazio do campo muda a posição (e a função) dele
    onDropPitch: (from, x, y) => {
      if (!t || from.kind !== 'starter') return
      const sel = moveOnPitch(t, from.index, x, y)
      if (sel) void apply(sel)
    },
  })

  const cancel = async () => {
    if (changed.current && original.current) {
      try {
        await api.setTactics(original.current)
        const info = await api.gameInfo()
        if (info) game.update(info)
      } catch (e) {
        return setError(String(e))
      }
    }
    nav.back()
  }

  const label = (p: TacticPlayer | null | undefined, fallback: string) => (p ? surname(p.listName) : fallback)
  const toggler = (p: TacticPlayer) => (editable ? () => void toggle(p.id) : undefined)

  return (
    <div className="flex h-full flex-col">
      <TitleBand title={name && `Tácticas ${name}`} fg={colours.fg} bg={colours.bg} />
      <div className="flex shrink-0 items-center gap-1 px-1.5">
        <PurpleMenu
          label="Tácticas"
          options={TACTICS_MENU}
          value={t && !t.custom ? String(t.formationIndex) : undefined}
          disabled={!editable}
          onChange={(k) => {
            if (k === 'instructions') setInstructions(true)
            else if (t) void apply({ ...selectionOf(t), formation: Number(k), positions: [], auto: false })
          }}
        />
        <PurpleMenu label="Vêr" options={[]} disabled />
        <div className="flex-1" />
        {t?.auto && editable && <span className="px-2 text-[12px] text-cm-yellow">O adjunto escolhe a equipa</span>}
        <button className="cm-purple h-6 w-40" disabled title="Chega com o relato dos jogos (3.4)">
          Último Jogo
        </button>
        <PurpleMenu
          label="Editar"
          align="right"
          options={[
            { key: 'auto', label: 'Adjunto escolhe a equipa' },
            { key: 'reset', label: 'Repor as posições da táctica' },
          ]}
          disabled={!editable}
          onChange={(k) => {
            if (k === 'auto' && t) void apply({ ...selectionOf(t), auto: true, positions: [] })
            if (k === 'reset' && t) void apply({ ...selectionOf(t), positions: [] })
          }}
        />
      </div>
      {error && <div className="mx-1.5 mt-1.5 bg-red-900 px-2 py-1 text-white">{error}</div>}
      {t && (
        <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 gap-1.5">
          <div className="flex min-h-0 w-[50%] flex-col">
            <div className="flex h-10 shrink-0 items-center justify-center gap-2 bg-black/70 font-title text-[20px] font-bold text-cm-yellow">
              {t.formation}
              {t.custom && '*'}
              {t.coach && (
                <span className="text-[14px] font-normal text-white">
                  (
                  <Link onClick={() => nav.go({ kind: 'person', id: t.coachId! })} className="text-cm-magenta">
                    {t.coach}
                  </Link>
                  )
                </span>
              )}
            </div>
            <div className="min-h-0 flex-1 overflow-auto bg-black/50 py-1">
              {t.starters.map((s, i) => {
                const spot: Spot = { kind: 'starter', index: i }
                return s.player ? (
                  <Row
                    key={`s${i}`}
                    p={s.player}
                    kind="starter"
                    place={shortRole(s.role)}
                    bind={drag.bind(spot, label(s.player, s.role))}
                    onToggle={toggler(s.player)}
                    onOpen={() => nav.go({ kind: 'person', id: s.player!.id })}
                  />
                ) : (
                  <div key={`s${i}`} {...drag.bind(spot, s.role)} className="flex h-[19px] items-center gap-2 px-1 text-[13px]">
                    <span className="w-7 shrink-0" />
                    <span className="cm-cell w-9 shrink-0 text-center text-[11px] text-cm-yellow">{shortRole(s.role)}</span>
                    <span className="italic text-[#8a8a8a]">— vazio: arraste alguém para aqui —</span>
                  </div>
                )
              })}
              <div className="my-0.5 border-t border-cm-line" />
              {/* Banco fixo de 9: as casas vazias também recebem jogadores arrastados */}
              {Array.from({ length: editable ? BENCH : t.bench.length }, (_, i) => {
                const p = t.bench[i]
                const spot: Spot = { kind: 'bench', index: i }
                return p ? (
                  <Row
                    key={`b${p.id}`}
                    p={p}
                    kind="bench"
                    place={`SB${i + 1}`}
                    bind={drag.bind(spot, label(p, ''))}
                    onToggle={toggler(p)}
                    onOpen={() => nav.go({ kind: 'person', id: p.id })}
                  />
                ) : (
                  <div key={`b-empty${i}`} {...drag.bind(spot, `SB${i + 1}`)} className="flex h-[19px] items-center gap-2 px-1 text-[13px]">
                    <span className="cm-cell w-7 shrink-0 !bg-[#1d2d8a]">&nbsp;</span>
                    <span className="cm-cell w-9 shrink-0 text-center text-[11px] text-cm-yellow">SB{i + 1}</span>
                    <span className="italic text-[#6a5a8a]">— vazio —</span>
                  </div>
                )
              })}
              {editable && t.reserves.length > 0 && (
                <>
                  <div className="my-0.5 border-t border-cm-line" />
                  {t.reserves.map((p) => (
                    <Row
                      key={`r${p.id}`}
                      p={p}
                      kind="reserve"
                      place=""
                      bind={drag.bind({ kind: 'reserve', id: p.id }, label(p, ''))}
                      onToggle={toggler(p)}
                      onOpen={() => nav.go({ kind: 'person', id: p.id })}
                    />
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
            {editable && (
              <div className="shrink-0 pt-1 text-center text-[12px] text-[#aab3c4]">
                Arraste para trocar ou para um lugar vazio · botão direito e arrastar faz a seta (atacar/recuar) · clique no número para pôr/tirar
              </div>
            )}
          </div>
        </div>
      )}
      {!t && <div className="flex-1" />}
      {drag.ghost}
      {instructions && t && (
        <InstructionsDialog
          initial={t.instructions}
          onCancel={() => setInstructions(false)}
          onOk={(i) => {
            setInstructions(false)
            void apply({ ...selectionOf(t), instructions: i })
          }}
        />
      )}
      <BottomBar left="Cancelar" right="Ok" onLeft={() => void cancel()} onRight={() => nav.back()} />
    </div>
  )
}
