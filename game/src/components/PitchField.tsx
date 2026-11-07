import type { HTMLAttributes, ReactNode } from 'react'

/** O relvado com as linhas, na proporção de um campo a sério: de pé (7 x 10, o ataque para
 * cima) ou deitado (`landscape`, 10 x 7, o ataque para a direita). Os filhos põem-se por cima,
 * em percentagem (0–100) da largura e da altura. */
export default function PitchField({
  children,
  className = '',
  style,
  landscape,
  ...rest
}: HTMLAttributes<HTMLDivElement> & { children?: ReactNode; landscape?: boolean }) {
  const line = 'pointer-events-none absolute border-white/25'
  return (
    <div
      {...rest}
      className={`relative overflow-hidden rounded-md shadow-[inset_0_0_60px_rgba(0,0,0,0.45)] ${className}`}
      style={{
        aspectRatio: landscape ? '10 / 7' : '7 / 10',
        background: `repeating-linear-gradient(${landscape ? '90deg' : '0deg'}, rgba(255,255,255,0.025) 0 7.5%, transparent 7.5% 15%), radial-gradient(ellipse at 50% 45%, #2a6e47, #173f2b 75%)`,
        ...style,
      }}
    >
      <div className={`${line} inset-[4%] rounded-[2px] border-2`} />
      {landscape ? (
        <>
          <div className={`${line} bottom-[4%] left-1/2 top-[4%] border-l-2`} />
          <div className={`${line} left-1/2 top-1/2 aspect-square h-[26%] -translate-x-1/2 -translate-y-1/2 rounded-full border-2`} />
          <div className={`${line} left-1/2 top-1/2 size-1.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-white/40`} />
          <div className={`${line} left-[4%] top-1/2 h-[54%] w-[16%] -translate-y-1/2 border-2 border-l-0`} />
          <div className={`${line} left-[4%] top-1/2 h-[26%] w-[6%] -translate-y-1/2 border-2 border-l-0`} />
          <div className={`${line} left-[20%] top-1/2 aspect-[1/2] h-[20%] -translate-y-1/2 rounded-r-full border-2 border-l-0`} />
          <div className={`${line} right-[4%] top-1/2 h-[54%] w-[16%] -translate-y-1/2 border-2 border-r-0`} />
          <div className={`${line} right-[4%] top-1/2 h-[26%] w-[6%] -translate-y-1/2 border-2 border-r-0`} />
          <div className={`${line} right-[20%] top-1/2 aspect-[1/2] h-[20%] -translate-y-1/2 rounded-l-full border-2 border-r-0`} />
        </>
      ) : (
        <>
          <div className={`${line} left-[4%] right-[4%] top-1/2 border-t-2`} />
          <div className={`${line} left-1/2 top-1/2 aspect-square w-[26%] -translate-x-1/2 -translate-y-1/2 rounded-full border-2`} />
          <div className={`${line} left-1/2 top-1/2 size-1.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-white/40`} />
          <div className={`${line} left-1/2 top-[4%] h-[16%] w-[54%] -translate-x-1/2 border-2 border-t-0`} />
          <div className={`${line} left-1/2 top-[4%] h-[6%] w-[26%] -translate-x-1/2 border-2 border-t-0`} />
          <div className={`${line} left-1/2 top-[20%] aspect-[2/1] w-[20%] -translate-x-1/2 rounded-b-full border-2 border-t-0`} />
          <div className={`${line} bottom-[4%] left-1/2 h-[16%] w-[54%] -translate-x-1/2 border-2 border-b-0`} />
          <div className={`${line} bottom-[4%] left-1/2 h-[6%] w-[26%] -translate-x-1/2 border-2 border-b-0`} />
          <div className={`${line} bottom-[20%] left-1/2 aspect-[2/1] w-[20%] -translate-x-1/2 rounded-t-full border-2 border-b-0`} />
        </>
      )}
      {children}
    </div>
  )
}
