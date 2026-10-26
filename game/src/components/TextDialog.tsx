import { useState } from 'react'
import { DialogFrame, Overlay } from './Dialog'

/** Diálogo verde com um texto a escrever (Definir Alcunha, Criar Nota Treinador). */
export default function TextDialog({
  title,
  initial,
  multiline,
  onCancel,
  onOk,
}: {
  title: string
  initial: string
  multiline?: boolean
  onCancel: () => void
  onOk: (text: string) => void
}) {
  const [text, setText] = useState(initial)
  const field = 'w-full border border-[#1d3a1f] bg-black px-2 py-1 text-[17px] text-cm-yellow outline-none'
  return (
    <Overlay>
      <DialogFrame title={title} width={560} onCancel={onCancel} onOk={() => onOk(text)}>
        {multiline ? (
          <textarea autoFocus rows={5} value={text} onChange={(e) => setText(e.target.value)} className={field} />
        ) : (
          <input
            autoFocus
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && onOk(text)}
            className={field}
          />
        )}
        <div className="pt-1 text-[13px] text-[#bfe6c4]">Em branco apaga.</div>
      </DialogFrame>
    </Overlay>
  )
}
