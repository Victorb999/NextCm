// "Comparar Jogadores": o primeiro jogador escolhido no menu Acção, à espera do segundo.

let first: { id: number; name: string } | null = null

export const compareFirst = () => first
export const setCompareFirst = (p: { id: number; name: string } | null) => {
  first = p
}
