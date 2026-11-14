import { FLAGS } from '../flags'

/** A bandeira de uma nação (pelo nome do banco ou do jogo), à altura do texto; nada se não houver.
 * `round`: redonda (o recorte quadrado do flag-icons num círculo). */
export default function NationFlag({
  name,
  className = '',
  round = false,
}: {
  name: string | null | undefined
  className?: string
  round?: boolean
}) {
  const code = name ? FLAGS[name] : undefined
  if (!code) return null
  const shape = round ? 'fis size-[1em] rounded-full' : 'rounded-[2px]'
  return (
    <span
      className={`fi fi-${code} ${shape} inline-block shrink-0 align-[-0.125em] shadow-[0_0_0_1px_rgba(0,0,0,0.5)] ${className}`}
      aria-hidden="true"
    />
  )
}
