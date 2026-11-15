import { useState } from 'react'
import { api } from '../../api'
import type { EditorHit } from '../../bindings/EditorHit'
import { DialogFrame, Overlay } from '../../components/Dialog'
import { ErrorBox } from '../../components/Screen'
import { EMPTY_FILTER } from './editorEdits'

/** Escolher um clube ou uma pessoa (jogador ou staff) do save: para os favoritos, os rivais e o
 * clube dono do passe. Procura pelo nome, como a procura da janela principal. */
export default function PickDialog({
  file,
  kind,
  title,
  onPick,
  onClose,
}: {
  file: string
  kind: 'club' | 'person'
  title: string
  onPick: (hit: { id: number; name: string }) => void
  onClose: () => void
}) {
  const [query, setQuery] = useState('')
  const [hits, setHits] = useState<EditorHit[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [picked, setPicked] = useState<EditorHit | null>(null)

  const search = async () => {
    setError(null)
    try {
      if (kind === 'club') {
        setHits(await api.editorSearchClubs(file, query))
      } else {
        const [players, staff] = await Promise.all([api.editorSearch(file, { ...EMPTY_FILTER, query }), api.editorSearchStaff(file, query)])
        setHits([...players, ...staff])
      }
      setPicked(null)
    } catch (e) {
      setError(String(e))
    }
  }

  return (
    <Overlay>
      <DialogFrame
        title={title}
        width={520}
        onCancel={onClose}
        onOk={picked ? () => onPick({ id: picked.id, name: picked.name }) : undefined}
      >
        <ErrorBox className="mb-2">{error}</ErrorBox>
        <form
          className="mb-2 flex gap-1"
          onSubmit={(e) => {
            e.preventDefault()
            void search()
          }}
        >
          <input
            autoFocus
            className="cm-field h-7 min-w-0 flex-1 text-[14px]"
            placeholder={kind === 'club' ? 'Nome do clube' : 'Nome do jogador ou do staff'}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <button className="cm-purple h-7 w-28 text-[13px]">Procurar</button>
        </form>
        <div className="cm-zebra h-72 overflow-auto rounded border border-white/15 bg-black/30">
          {hits?.map((h) => (
            <button
              key={h.id}
              onClick={() => setPicked(h)}
              onDoubleClick={() => onPick({ id: h.id, name: h.name })}
              className={`flex h-6 w-full items-center gap-2 px-2 text-left text-[13.5px] ${picked?.id === h.id ? 'bg-cm-sel text-white' : 'text-cm-yellow'}`}
            >
              <span className="min-w-0 flex-1 truncate">{h.name}</span>
              <span className="w-40 truncate text-right text-[12px] text-cm-lilac">{h.club ?? ''}</span>
            </button>
          ))}
          {hits?.length === 0 && <div className="p-3 text-center text-[13px] text-cm-lilac">Ninguém encontrado.</div>}
        </div>
      </DialogFrame>
    </Overlay>
  )
}
