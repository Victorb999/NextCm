/** Uma camisola nas cores do clube (a faixa do CM: fundo e texto), para a ficha do Novo Jogo. */
export default function Shirt({ bg, fg, size = 64 }: { bg: string | null; fg: string | null; size?: number }) {
  const body = bg ?? '#888888'
  const trim = fg ?? '#ffffff'
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
      <path
        d="M22 6 L12 10 L3 22 L11 28 L15 24 L15 58 L49 58 L49 24 L53 28 L61 22 L52 10 L42 6 Q32 14 22 6 Z"
        fill={body}
        stroke="rgba(0,0,0,0.55)"
        strokeWidth="1.5"
        strokeLinejoin="round"
      />
      {/* Gola e punhos na cor do texto */}
      <path d="M22 6 Q32 14 42 6" fill="none" stroke={trim} strokeWidth="3" />
      <path d="M3 22 L11 28" stroke={trim} strokeWidth="3" />
      <path d="M61 22 L53 28" stroke={trim} strokeWidth="3" />
    </svg>
  )
}
