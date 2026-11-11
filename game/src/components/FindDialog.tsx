import { useEffect, useState } from 'react'
import { api } from '../api'
import type { FindResult } from '../bindings/FindResult'
import type { Screen } from '../nav'
import { DialogFrame, Overlay } from './Dialog'

/** Uma linha do resultado: o nome e, ao lado, o que o distingue (nação, clube, divisão). */
type Hit = { key: string; name: string; detail?: string | null; to: Screen }

/** O Procurar do CM: escreve-se um nome e aparecem os clubes, nações, competições, jogadores e staff. */
export default function FindDialog({
  onGo,
  onAdvanced,
  onClose,
}: {
  onGo: (s: Screen) => void
  /** A Pesquisa Jogador & Staff, com os filtros. */
  onAdvanced: () => void
  onClose: () => void
}) {
  const [text, setText] = useState('')
  const [found, setFound] = useState<FindResult | null>(null)

  // Procura enquanto se escreve (com uma pausa curta, para não pedir a cada letra)
  useEffect(() => {
    const t = setTimeout(() => {
      if (text.trim().length < 2) return setFound(null)
      api.find(text).then(setFound, () => setFound(null))
    }, 200)
    return () => clearTimeout(t)
  }, [text])

  const groups: [string, Hit[]][] = found
    ? [
        [
          'Clubes',
          found.clubs.map((c) => ({
            key: `c${c.club.id}`,
            name: c.club.name,
            detail: [c.nation, c.division].filter(Boolean).join(' · '),
            to: { kind: 'club', id: c.club.id },
          })),
        ],
        ['Nações', found.nations.map((n) => ({ key: `n${n.id}`, name: n.name, to: { kind: 'nation', id: n.id } }))],
        [
          'Competições',
          found.competitions.map((c) => ({ key: `k${c.id}`, name: c.name, to: { kind: 'competition', id: c.id } })),
        ],
        [
          'Jogadores',
          found.players.map((p) => ({
            key: `p${p.id}`,
            name: p.listName,
            detail: [p.position, p.club ?? 'Sem clube'].filter(Boolean).join(' · '),
            to: { kind: 'person', id: p.id },
          })),
        ],
        [
          'Staff',
          found.staff.map((p) => ({
            key: `s${p.id}`,
            name: p.listName,
            detail: [p.job, p.club].filter(Boolean).join(' · '),
            to: { kind: 'person', id: p.id },
          })),
        ],
      ]
    : []
  const empty = found && groups.every(([, hits]) => hits.length === 0)

  return (
    <Overlay>
      <DialogFrame
        title="Procurar"
        width={720}
        onCancel={onClose}
        okLabel="Pesquisa Avançada"
        onOk={onAdvanced}
        header={
          <input
            autoFocus
            value={text}
            placeholder="Nome do clube, nação, competição, jogador ou staff…"
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => e.key === 'Escape' && onClose()}
            className="mt-1.5 w-full rounded border border-[#1d3a1f] bg-black px-2 py-1 text-[17px] text-cm-yellow outline-none"
          />
        }
      >
        <div className="h-[420px]">
          {!found && <div className="text-[14px] text-cm-dlgtext">Escreva pelo menos duas letras.</div>}
          {empty && <div className="text-[14px] text-cm-dlgtext">Nada encontrado com esse nome.</div>}
          {groups
            .filter(([, hits]) => hits.length > 0)
            .map(([label, hits]) => (
              <div key={label} className="mb-2">
                <div className="font-title text-[17px] leading-7 text-cm-yellow">{label}</div>
                {hits.map((h) => (
                  <button
                    key={h.key}
                    onClick={() => onGo(h.to)}
                    className="flex h-[24px] w-full items-center gap-3 px-2 text-left text-[14px] text-white hover:bg-cm-sel"
                  >
                    <span className="truncate">{h.name}</span>
                    {h.detail && <span className="ml-auto shrink-0 truncate text-cm-dlgtext">{h.detail}</span>}
                  </button>
                ))}
              </div>
            ))}
        </div>
      </DialogFrame>
    </Overlay>
  )
}
