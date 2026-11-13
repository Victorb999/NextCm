import type { ReactNode } from 'react'
import { PLAYER_ATTRIBUTES } from '../../attributes'
import type { PersonProfile } from '../../bindings/PersonProfile'
import type { PositionTag } from '../../bindings/PositionTag'
import { SeasonTable } from '../../components/PlayerHistory'
import { Empty, Panel } from '../../components/Screen'
import { cmDate } from '../../format'
import InfoRows from './InfoRows'

// Atributos na ordem e com os nomes do perfil do CM 01/02 em português.

const STAFF_ATTRIBUTES: [string, string][] = [
  ['adaptability', 'Adaptabilidade'],
  ['ambition', 'Ambição'],
  ['attacking', 'Ataque'],
  ['judging_potential', 'Avaliar Potencial'],
  ['judgement', 'Avaliar Jogadores'],
  ['determination', 'Determinação'],
  ['discipline', 'Disciplina'],
  ['physiotherapy', 'Fisioterapia'],
  ['offside', 'Fora de Jogo'],
  ['free_roles', 'Funções Livres'],
  ['man_handling', 'Gestão Humana'],
  ['directness', 'Jogo Directo'],
  ['youngsters', 'Jovens'],
  ['loyalty', 'Lealdade'],
  ['marking', 'Marcação'],
  ['motivating', 'Motivação'],
  ['patience', 'Paciência'],
  ['pressing', 'Pressing'],
  ['professionalism', 'Profissionalismo'],
  ['resources', 'Recursos'],
  ['tactics', 'Táctica'],
  ['coaching', 'Treinar'],
  ['coaching_gks', 'Treinar GR'],
  ['coaching_technique', 'Treinar Técnica'],
]

/** A cor de um atributo pela faixa, como no CM: fraco, razoável, bom, excelente. */
function valueColor(v: number) {
  if (v >= 16) return 'text-[#ff3b3b]'
  if (v >= 11) return 'text-cm-orange'
  if (v >= 6) return 'text-[#f5e663]'
  return 'text-[#8f8fa0]'
}

/** Grade de 3 colunas "Nome .... valor", como no perfil do CM. */
/** `sep`: a linha abre um bloco à parte (a Forma, a Moral e a Condição depois dos atributos). */
function AttributeGrid({ items }: { items: { label: string; value: ReactNode; color?: string; sep?: boolean }[] }) {
  const perColumn = Math.ceil(items.length / 3)
  const columns = [0, 1, 2].map((c) => items.slice(c * perColumn, (c + 1) * perColumn))
  return (
    <div className="grid shrink-0 grid-cols-3 gap-2">
      {columns.map((col, c) => (
        <div key={c} className="cm-zebra overflow-hidden rounded-md bg-cm-surface">
          {col.map((it) => (
            <div
              key={it.label}
              className={`flex h-6 items-center justify-between px-3 ${it.sep ? 'mt-1.5 border-t-2 border-cm-yellow/40' : ''}`}
            >
              <span className="text-[14.5px] text-cm-text">{it.label}</span>
              <span className={`text-[14.5px] font-bold tabular-nums ${it.color ?? 'text-cm-orange'}`}>{it.value}</span>
            </div>
          ))}
        </div>
      ))}
    </div>
  )
}

/** Tabela pequena de cabeçalho roxo (a época quando não há dados do jogo). */
function PurpleTable({ head, rows }: { head: string[]; rows: ReactNode[][] }) {
  return (
    <table className="w-full overflow-hidden rounded-md border-collapse bg-cm-surface text-center">
      <thead>
        <tr className="h-8">
          {head.map((h, i) => (
            <th
              key={i}
              className={`bg-cm-headdeep px-3 font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-cm-yellow ${i === 0 ? 'text-left' : ''}`}
            >
              {h}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((r, i) => (
          <tr key={i} className="h-[28px] text-[14.5px]">
            {r.map((cell, j) => (
              <td key={j} className={`px-3 tabular-nums ${j === 0 ? 'text-left text-white' : 'text-cm-orange'}`}>
                {cell}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  )
}

// Onde fica cada posição no campo pequeno (ataque para a direita, esquerda em cima)
const ROLE_X: Record<string, number> = { GR: 6, L: 16, D: 25, AL: 40, MD: 42, M: 57, MA: 72, A: 88 }
const SIDE_Y: Record<string, number> = { E: 18, C: 50, D: 82 }

/** Campo pequeno com as posições em que joga (a de origem a cheio). */
function Pitch({ tags }: { tags: PositionTag[] }) {
  return (
    <div className="relative h-[118px] overflow-hidden rounded border border-cm-dlgline bg-cm-dlg">
      <div className="absolute inset-y-0 left-1/2 border-l border-cm-dlgline" />
      <div className="absolute left-1/2 top-1/2 size-9 -translate-x-1/2 -translate-y-1/2 rounded-full border border-cm-dlgline" />
      <div className="absolute left-0 top-1/2 h-12 w-5 -translate-y-1/2 border border-l-0 border-cm-dlgline" />
      <div className="absolute right-0 top-1/2 h-12 w-5 -translate-y-1/2 border border-r-0 border-cm-dlgline" />
      {tags.map((t) => (
        <span
          key={t.code}
          className={`absolute -translate-x-1/2 -translate-y-1/2 rounded px-1 font-mono text-[10px] font-bold ${
            t.natural ? 'bg-cm-dlgborder text-black' : 'border border-cm-dlgborder bg-cm-dlg text-cm-mint'
          }`}
          style={{ left: `${ROLE_X[t.role] ?? 50}%`, top: `${SIDE_Y[t.side ?? 'C']}%` }}
        >
          {t.code}
        </span>
      ))}
    </div>
  )
}

export default function Profile({ p }: { p: PersonProfile }) {
  const pl = p.player
  const attrs = pl?.attributes ?? p.staff?.attributes ?? {}
  const list = pl ? PLAYER_ATTRIBUTES : STAFF_ATTRIBUTES
  const items: { label: string; value: ReactNode; color?: string; sep?: boolean }[] = list.map(([key, label]) => {
    const v = attrs[key]
    return { label, value: v ?? '-', color: v ? valueColor(v) : 'text-[#777]' }
  })
  if (pl) {
    items.push(
      { label: 'Forma', sep: true, value: p.form.length ? p.form.map((r) => Math.round(r)).join('-') : '-' },
      { label: 'Moral', value: p.status?.moraleLabel ?? '-' },
      { label: 'Condição', value: `${p.status?.condition ?? 100}%` },
    )
  }
  const ability = pl ?? p.staff
  const total = p.season.reduce((t, s) => ({ apps: t.apps + s.apps, goals: t.goals + s.goals, assists: t.assists + s.assists }), {
    apps: 0,
    goals: 0,
    assists: 0,
  })
  const rating = total.apps ? p.season.reduce((t, s) => t + s.rating * s.apps, 0) / total.apps : 0

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-auto pb-1">
      <div className="flex shrink-0 gap-2">
        <div className="min-w-0 flex-1">
          <AttributeGrid items={items} />
        </div>
        <div className="flex w-[250px] shrink-0 flex-col gap-2">
          {pl && (
            <Panel title="Posições">
              <div className="p-2">
                <Pitch tags={pl.positions} />
              </div>
            </Panel>
          )}
          {p.assistantNote && (
            <Panel title="Relatório do Adjunto">
              <div className="px-3 py-2 text-[13.5px] italic leading-snug text-white">“{p.assistantNote}”</div>
            </Panel>
          )}
          <Panel title="Situação">
            <InfoRows
              rows={[
                ...(pl ? ([['Número', pl.squadNumber ?? '-']] as [string, ReactNode][]) : []),
                ['Rep. Actual', ability?.currentReputation ?? '-'],
                ['Rep. Mundial', ability?.worldReputation ?? '-'],
                ['No clube desde', cmDate(p.clubJoined)],
                ['Contrato até', cmDate(p.contractExpires)],
              ]}
            />
            {p.note && (
              <div className="border-t border-cm-line px-3 py-1.5 text-[13px] text-white">
                <span className="text-cm-yellow">Nota: </span>
                {p.note}
              </div>
            )}
          </Panel>
        </div>
      </div>
      {p.knowledge && p.knowledge !== 'known' && (
        <div className="shrink-0 rounded border border-cm-line bg-cm-panel px-3 py-1 text-center text-cm-yellow">
          {p.reportOn
            ? `Os seus olheiros estão a observá-lo: o relatório chega a ${cmDate(p.reportOn)}.`
            : p.knowledge === 'unknown'
              ? 'O seu clube não conhece este jogador. Mande observá-lo (Acção → Observar Jogador).'
              : 'O seu clube conhece este jogador só em parte. Mande observá-lo para ver tudo.'}
        </div>
      )}
      {pl && p.seasons[0]?.byKind.length ? (
        <Panel title={`Estatísticas da Época ${p.seasons[0].label}`} className="shrink-0">
          <SeasonTable byKind={p.seasons[0].byKind} />
        </Panel>
      ) : (
        pl && (
          <Panel title="Estatísticas da Época" className="shrink-0">
            {p.season.length === 0 ? (
              <Empty className="p-3">Ainda não jogou esta época.</Empty>
            ) : (
              <PurpleTable
                head={['Competição', 'Jogos', 'Golos', 'Asts', 'Nota']}
                rows={[
                  ...p.season.map((s) => [s.competition, s.apps, s.goals, s.assists, s.rating.toFixed(2)]),
                  [
                    <b key="t" className="text-cm-mint">
                      Total
                    </b>,
                    total.apps,
                    total.goals,
                    total.assists,
                    rating.toFixed(2),
                  ],
                ]}
              />
            )}
          </Panel>
        )
      )}
    </div>
  )
}
