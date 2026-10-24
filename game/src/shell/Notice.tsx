/** Aviso amarelo no topo ("Jogo gravado.", erros); some sozinho (`useNotice`). */
export default function Notice({ text }: { text: string }) {
  return (
    <div className="absolute left-1/2 top-16 z-50 -translate-x-1/2 border-2 border-black bg-cm-yellow px-4 py-1 font-title text-[17px] font-bold text-black">
      {text}
    </div>
  )
}
