import { useState } from 'react'
import type { EditorPreference } from '../../bindings/EditorPreference'
import { BOX } from './editorText'
import { Header, RefList } from './Fields'
import PickDialog from './PickDialog'

type Group = { kind: 'club' | 'person'; liked: boolean; title: string }
const GROUPS: Group[] = [
  { kind: 'club', liked: true, title: 'Clubes favoritos' },
  { kind: 'club', liked: false, title: 'Clubes detestados' },
  { kind: 'person', liked: true, title: 'Pessoas favoritas' },
  { kind: 'person', liked: false, title: 'Pessoas detestadas' },
]

/** As Preferências do jogador (os favoritos e os detestados do CM): 3 clubes e 3 pessoas de cada. */
export default function PrefsTab({
  file,
  list,
  onChange,
}: {
  file: string
  list: EditorPreference[]
  onChange: (list: EditorPreference[]) => void
}) {
  const [adding, setAdding] = useState<Group | null>(null)
  const of = (g: Group) => list.filter((f) => f.kind === g.kind && f.liked === g.liked)
  const drop = (g: Group, id?: number) =>
    onChange(list.filter((f) => f.kind !== g.kind || f.liked !== g.liked || (id !== undefined && f.targetId !== id)))
  return (
    <>
      <div className="flex max-w-3xl flex-col gap-2">
        {(['club', 'person'] as const).map((kind) => (
          <div key={kind} className={BOX}>
            <Header>{kind === 'club' ? 'Clubes' : 'Jogadores e staff'}</Header>
            <div className="flex gap-3">
              {GROUPS.filter((g) => g.kind === kind).map((g) => (
                <RefList
                  key={g.title}
                  title={g.title}
                  items={of(g).map((f) => ({ id: f.targetId, name: f.name }))}
                  onAdd={() => setAdding(g)}
                  onRemove={(id) => drop(g, id)}
                  onClear={() => drop(g)}
                />
              ))}
            </div>
          </div>
        ))}
        <div className="text-[12px] text-cm-dim">
          Os clubes favoritos atraem o jogador, e ele se recusa a ir para os detestados; as pessoas contam nas relações do elenco.
        </div>
      </div>
      {adding && (
        <PickDialog
          file={file}
          kind={adding.kind}
          title={adding.title}
          onClose={() => setAdding(null)}
          onPick={(h) => {
            const g = adding
            setAdding(null)
            // Um clube ou pessoa fica numa só lista (favorito ou detestado)
            const rest = list.filter((f) => f.kind !== g.kind || f.targetId !== h.id)
            onChange([...rest, { kind: g.kind, targetId: h.id, name: h.name, liked: g.liked }])
          }}
        />
      )}
    </>
  )
}
