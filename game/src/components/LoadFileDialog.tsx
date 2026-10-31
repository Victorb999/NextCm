import { useEffect, useState } from 'react'
import type { SavedFile } from '../bindings/SavedFile'
import { DialogFrame, Overlay } from './Dialog'

/** "Carregar Táctica" / "Carregar Lista": os ficheiros gravados numa pasta ao lado dos saves. */
export default function LoadFileDialog({
  title,
  load,
  empty,
  onCancel,
  onOk,
}: {
  title: string
  load: () => Promise<SavedFile[]>
  /** O texto quando ainda não há nada gravado. */
  empty: string
  onCancel: () => void
  onOk: (file: string) => void
}) {
  const [files, setFiles] = useState<SavedFile[] | null>(null)
  const [chosen, setChosen] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    load().then(setFiles, (e) => setError(String(e)))
  }, [load])

  return (
    <Overlay>
      <DialogFrame title={title} width={560} onCancel={onCancel} onOk={chosen ? () => onOk(chosen) : undefined}>
        {error && <div className="text-[#ff7070]">{error}</div>}
        {files && files.length === 0 && <div className="py-2 text-[15px] text-[#bfe6c4]">{empty}</div>}
        <div className="cm-zebra max-h-[360px] overflow-auto">
          {files?.map((f) => (
            <button
              key={f.file}
              onClick={() => setChosen(f.file)}
              onDoubleClick={() => onOk(f.file)}
              className={`flex h-8 w-full items-center gap-2 px-2 text-left text-[15px] ${
                chosen === f.file ? 'bg-cm-sel font-bold text-white' : 'text-white hover:bg-white/10'
              }`}
            >
              <span className="min-w-0 flex-1 truncate">{f.name}</span>
            </button>
          ))}
        </div>
      </DialogFrame>
    </Overlay>
  )
}
