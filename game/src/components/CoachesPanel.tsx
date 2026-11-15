import { valueColor } from '../attributes'
import type { AreaStrength } from '../bindings/AreaStrength'
import type { Coach } from '../bindings/Coach'
import { useNav } from '../nav'
import { Link } from './Screen'

// Os preparadores na tela de Treino (no lugar do diálogo do CM): cada linha é um preparador com o
// atributo dele em cada área; clicar numa área dá ou tira a área dele. Em baixo, a força de cada área
// com quem a treina, que o core recalcula a cada clique. Grava-se no Ok da tela.

/** As áreas, pela ordem do core, com o atributo que conta em cada uma. */
const AREAS: { label: string; attribute: string }[] = [
  { label: 'Físico', attribute: 'Treinar' },
  { label: 'Tática', attribute: 'Tática' },
  { label: 'Finalização', attribute: 'Ataque' },
  { label: 'Técnica', attribute: 'Treinar Técnica' },
  { label: 'Goleiros', attribute: 'Treinar GR' },
]

const head = 'text-[11px] font-bold font-mono uppercase tracking-[0.1em] text-cm-dim'

export default function CoachesPanel({
  coaches,
  areas,
  strength,
  onToggle,
}: {
  coaches: Coach[]
  areas: Map<number, boolean[]>
  strength: AreaStrength[]
  onToggle: (coach: number, area: number) => void
}) {
  const nav = useNav()
  if (!coaches.length) {
    return (
      <div className="rounded-md bg-cm-surface p-4 text-white">
        O clube não tem preparadores: os jogadores treinam sozinhos e evoluem menos. Contrate um na Pesquisa Staff.
      </div>
    )
  }
  return (
    <div className="flex flex-col gap-2">
      <div className="rounded-md bg-cm-surface">
        <div className={`flex h-9 items-center gap-2 border-b border-cm-line bg-black/30 px-3 ${head}`}>
          <span className="min-w-40 flex-1">Preparador</span>
          <span className="w-36">Cargo</span>
          {AREAS.map((a) => (
            <span key={a.label} className="w-24 text-center leading-tight" title={`Conta o atributo ${a.attribute}`}>
              {a.label}
              <div className="text-[9.5px] normal-case tracking-normal text-cm-faint">{a.attribute}</div>
            </span>
          ))}
          <span className="w-14 text-center" title="Motivação: rende um pouco mais em todas as áreas">
            Motiv.
          </span>
          <span className="w-14 text-center" title="Jovens: os jogadores até 21 anos evoluem mais">
            Jovens
          </span>
        </div>
        <div className="cm-zebra">
          {coaches.map((c) => {
            const on = areas.get(c.id) ?? c.areas
            const count = on.filter(Boolean).length
            return (
              <div key={c.id} className="flex h-9 items-center gap-2 px-3">
                <span className="min-w-40 flex-1 truncate text-[15px] font-semibold text-white">
                  <Link onClick={() => nav.go({ kind: 'person', id: c.id })}>{c.name}</Link>
                  {count >= 4 && (
                    <span className="ml-2 text-[11px] font-normal text-cm-orange" title="Quem treina muitas áreas rende menos">
                      {count} áreas · rende {count === 4 ? '95' : '90'}%
                    </span>
                  )}
                </span>
                <span className="w-36 truncate text-[13px] text-cm-lilac">{c.job}</span>
                {AREAS.map((a, i) => {
                  const value = c.ratings[i] ?? 1
                  const selected = !!on[i]
                  return (
                    <button
                      key={a.label}
                      onClick={() => onToggle(c.id, i)}
                      aria-pressed={selected}
                      title={selected ? `Tirar ${a.label} de ${c.name}` : `Dar ${a.label} a ${c.name} (${a.attribute} ${value})`}
                      className={`flex h-7 w-24 items-center justify-center rounded font-mono text-[15px] font-bold ${
                        selected
                          ? `bg-black/50 ring-2 ring-inset ring-cm-yellow ${valueColor(value)}`
                          : 'bg-black/15 text-white/25 hover:bg-black/35 hover:text-white/60'
                      }`}
                    >
                      {value}
                    </button>
                  )
                })}
                <span className={`w-14 text-center font-mono font-bold ${valueColor(c.motivating)}`}>{c.motivating}</span>
                <span className={`w-14 text-center font-mono font-bold ${valueColor(c.youngsters)}`}>{c.youngsters}</span>
              </div>
            )
          })}
        </div>
        {/* A força de cada área com quem a treina agora */}
        <div className="flex h-12 items-center gap-2 border-t-2 border-cm-yellow/40 bg-black/30 px-3">
          <span className="min-w-40 flex-1 text-[14px] font-bold text-cm-yellow">Força da área</span>
          <span className="w-36" />
          {AREAS.map((a, i) => {
            const s = strength[i]
            const empty = s?.coaches === 0
            return (
              <span key={a.label} className="flex w-24 flex-col items-center leading-tight">
                <span className={`font-mono text-[16px] font-bold ${empty ? 'text-cm-red' : valueColor(s?.value ?? 1)}`}>
                  {s ? s.value : '-'}
                </span>
                <span className={`text-[11px] ${empty ? 'font-bold text-cm-red' : 'text-cm-text'}`}>{s?.label ?? ''}</span>
              </span>
            )
          })}
          <span className="w-14" />
          <span className="w-14" />
        </div>
      </div>
      <div className="rounded-md bg-cm-surface px-3 py-2 text-[13px] leading-relaxed text-cm-text">
        Clique num número para dar ou tirar a área ao preparador. Cada área usa o atributo dele nela (o nome por baixo da coluna). Contam os
        três melhores de cada área (nos Goleiros, só o melhor); uma área sem ninguém treina mal. Quem treina 4 áreas rende 95%, e 5 áreas,
        90%. A Motivação dá um pouco mais de rendimento, e o melhor em Jovens faz os jogadores até 21 anos evoluírem mais. As mudanças valem
        no Ok.
      </div>
    </div>
  )
}
