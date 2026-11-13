import { useState } from 'react'
import type { ReactNode } from 'react'
import type { ClubPreview } from '../../bindings/ClubPreview'
import { api } from '../../api'
import Shirt from '../../components/Shirt'
import { ErrorBox } from '../../components/Screen'
import { money } from '../../format'
import { useLoad } from '../../useLoad'

/** A ficha de um clube no Novo Jogo, como a do "Select Team" do FM08. */
export default function ClubCard({ id }: { id: number | null }) {
  const [p, setP] = useState<ClubPreview | null>(null)
  const [error, setError] = useState<string | null>(null)
  useLoad(
    () => (id === null ? Promise.resolve(null) : api.clubPreview(id)),
    [id],
    (v) => {
      setP(v)
      setError(null)
    },
    setError,
  )
  if (id === null) return <div className="p-6 text-center text-cm-dlgtext">Escolha um clube na lista.</div>
  if (error) return <ErrorBox className="m-2">{error}</ErrorBox>
  if (!p || p.club.id !== id) return null

  const rows: [string, ReactNode][] = [
    ['Divisão', p.division ?? '-'],
    ['Reputação', p.reputation],
    ['Finanças', `${p.finances} (${money(p.cash)})`],
    ['Estádio', p.stadium ?? '-'],
    ['Capacidade', p.capacity ?? '-'],
    ['Instalações de Treino', p.facilities ?? '-'],
    ['Época Passada', p.lastSeason ?? '-'],
    ['Previsão dos Media', p.prediction ? `${p.prediction}º de ${p.clubsInDivision}` : '-'],
    ['A Direcção espera', p.boardGoal],
  ]
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div
        className="flex h-9 shrink-0 items-center justify-center rounded font-title text-[20px]"
        style={{ background: p.club.bg ?? undefined, color: p.club.fg ?? undefined }}
      >
        {p.club.name}
      </div>
      <div className="cm-zebra mt-2">
        {rows.map(([k, v]) => (
          <div key={k} className="flex min-h-6 items-center gap-3 px-3 text-[13.5px]">
            <span className="w-44 shrink-0 text-cm-dlgtext">{k}</span>
            <span className="min-w-0 flex-1 font-semibold text-cm-green">{v}</span>
          </div>
        ))}
      </div>
      <div className="mt-2 flex items-start gap-4 px-3">
        <div className="min-w-0 flex-1">
          <div className="pb-1 font-mono text-[11px] font-bold uppercase tracking-[0.12em] text-cm-orange">Jogadores-chave</div>
          {p.keyPlayers.map(([pid, name]) => (
            <div key={pid} className="truncate text-[14px] font-semibold text-white">
              {name}
            </div>
          ))}
        </div>
        <div className="flex shrink-0 items-end gap-4 text-center text-[12px] text-cm-dlgtext">
          <div>
            <Shirt bg={p.club.bg} fg={p.club.fg} />
            Casa
          </div>
          <div>
            <Shirt bg={p.awayBg} fg={p.awayFg} />
            Fora
          </div>
        </div>
      </div>
    </div>
  )
}
