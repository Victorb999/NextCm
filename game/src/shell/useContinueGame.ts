import { useState } from 'react'
import { api } from '../api'
import type { LineupCheck } from '../bindings/LineupCheck'
import { autosaveDue, getSettings } from '../settings'
import { useGame } from '../game'
import { useNav } from '../nav'
import type { MatchDayData } from './types'

/**
 * O botão "Continuar Jogo": avança o dia (ou volta ao jogo do treinador por acabar), abre o
 * dia de jogo quando é hoje e, como no CM, a caixa de mensagens quando há notícias por ler.
 * Com um dia de jogo aberto, `matchStep` faz o papel do Seguinte e o dia não avança.
 */
export function useContinueGame({
  notify,
  onProcessing,
  onMatch,
  matchStep,
  onLineup,
}: {
  notify: (msg: string) => void
  onProcessing: (busy: boolean) => void
  onMatch: (m: MatchDayData) => void
  matchStep?: () => void
  /** Antes do dia de jogo: o que falta na equipa ou o que confirmar; `go` avança mesmo assim. */
  onLineup: (check: LineupCheck, go: () => void) => void
}) {
  const nav = useNav()
  const game = useGame()
  const [busy, setBusy] = useState(false)
  const info = game.mode === 'game' ? game.info : null

  const advance = async (confirmed = false) => {
    if (matchStep) return matchStep()
    if (busy) return
    setBusy(true)
    onProcessing(true)
    try {
      // Com um jogo do treinador por acabar (ex.: a página recarregou), volta a ele
      let next = info
      // Convocados sem número para o jogo de amanhã: primeiro os números, como no CM
      if (next?.needsNumbers) {
        nav.go({ kind: 'squadNumbers' })
        return
      }
      if (!next?.live) {
        // Amanhã há jogo e a equipa é do treinador: o aviso ou a confirmação do CM
        const check = confirmed ? null : await api.lineupCheck()
        if (check && (check.problem || check.warnings.length > 0)) {
          onLineup(check, () => void advance(true))
          return
        }
        const before = next?.date
        next = await api.continueGame()
        game.update(next)
        // Auto Gravação (Opções Jogo): fora dos dias de jogo
        if (before && !next.live && autosaveDue(getSettings().autosave, before, next.date)) {
          game.update(await api.saveGame())
        }
      }
      if (next.live) {
        // O jogo do treinador é hoje: os resultados do dia só entram no fim dele, então a
        // tabela e os jogos do dia ainda estão como antes da jornada
        const { fixture } = await api.liveMatch()
        const [all, table] = await Promise.all([api.listFixtures(fixture.competitionId), api.leagueTable(fixture.competitionId)])
        onMatch({ fixture, dayGames: all.filter((f) => f.date === fixture.date), table })
      } else if (next.unreadNews > 0) {
        nav.go({ kind: 'news' })
      }
    } catch (e) {
      notify(String(e))
    } finally {
      setBusy(false)
      onProcessing(false)
    }
  }

  return { advance, busy }
}
