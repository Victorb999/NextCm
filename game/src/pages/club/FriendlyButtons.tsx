import { useState } from 'react'
import { api } from '../../api'
import type { Fixture } from '../../bindings/Fixture'
import { DialogFrame, Overlay } from '../../components/Dialog'
import { PurpleMenu } from '../../components/Screen'
import { shortDate } from '../../format'
import { CLUB_FRIENDLY, setFriendlyPlan, useFriendlyPlan } from '../../friendlyPlan'
import { useGame } from '../../game'
import { useNav } from '../../nav'

// Os amigáveis na tela do clube, como no CM: "Convidar Clube" (com um amistoso por marcar) e,
// no Calendário do clube do treinador, "Cancelar" e "Marcar".

/** O botão vermelho "Convidar Clube" na faixa de outro clube, com a resposta num diálogo verde. */
export function InviteButton({ clubId }: { clubId: number }) {
  const plan = useFriendlyPlan()
  const game = useGame()
  const nav = useNav()
  const [reply, setReply] = useState<{ ok: boolean; text: string } | null>(null)
  const own = game.info?.club.id
  if ((!plan && !reply) || game.mode !== 'game' || clubId === own) return null

  const invite = async () => {
    if (!plan) return
    try {
      const [f] = await api.arrangeFriendlies(plan.kind, [{ date: plan.date, club: clubId }])
      setFriendlyPlan(null)
      const other = f ? (f.home.id === own ? f.away : f.home).shortName : ''
      setReply({ ok: true, text: `O ${other} aceitou o convite: o amigável fica marcado para ${f ? shortDate(f.date) : ''}.` })
    } catch (e) {
      setReply({ ok: false, text: String(e) })
    }
  }
  return (
    <>
      {plan && (
        <button
          className="h-8 rounded bg-cm-red px-4 font-title text-[15px] font-bold text-black hover:brightness-110"
          title={`${plan.kind === 'home' ? 'Amistoso em Casa' : 'Amistoso Fora'} a ${shortDate(plan.date)}`}
          onClick={() => void invite()}
        >
          Convidar Clube
        </button>
      )}
      {reply && (
        <Overlay>
          <DialogFrame
            title="Convite"
            width={520}
            cancelLabel={reply.ok ? 'Fechar' : 'Desistir'}
            onCancel={() => {
              if (!reply.ok) setFriendlyPlan(null)
              setReply(null)
            }}
            okLabel={reply.ok ? 'Ver Calendário' : 'Outro Clube'}
            onOk={() => {
              setReply(null)
              if (reply.ok && own !== undefined) nav.go({ kind: 'club', id: own, tab: 'fixtures' })
            }}
          >
            <div className="py-2 text-[17px] text-white">{reply.text}</div>
          </DialogFrame>
        </Overlay>
      )}
    </>
  )
}

/** "Cancelar" (o amigável escolhido na lista) e "Marcar" (o menu do CM) no Calendário. */
export function FriendlyActions({ selected, onCancelled }: { selected: Fixture | null; onCancelled: () => void }) {
  const nav = useNav()
  const [error, setError] = useState<string | null>(null)
  const cancellable = selected !== null && selected.competitionId === CLUB_FRIENDLY && selected.homeGoals === null
  const cancel = async () => {
    if (!selected) return
    try {
      await api.cancelFriendly(selected.id)
      setError(null)
      onCancelled()
    } catch (e) {
      setError(String(e))
    }
  }
  return (
    <>
      {error && <span className="truncate text-[12px] text-cm-red">{error}</span>}
      <button className="cm-purple h-6 w-28" disabled={!cancellable} onClick={() => void cancel()}>
        Cancelar
      </button>
      <PurpleMenu
        label="Marcar"
        align="right"
        buttonClass="cm-purple flex h-6 w-28 items-center justify-center gap-1"
        options={[
          { key: 'home', label: 'Amistoso em Casa' },
          { key: 'away', label: 'Amistoso Fora' },
          { key: 'tour', label: 'Excursão' },
        ]}
        onChange={(mode) => nav.go({ kind: 'friendly', mode })}
      />
    </>
  )
}
