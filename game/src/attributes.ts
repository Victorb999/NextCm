// Atributos de jogador na ordem e com os nomes do perfil do CM 01/02 em português (perfil e Comparar Jogadores).

export const PLAYER_ATTRIBUTES: [string, string][] = [
  ['acceleration', 'Aceleração'],
  ['agility', 'Agilidade'],
  ['aggression', 'Agressividade'],
  ['anticipation', 'Antecipação'],
  ['balance', 'Balanço'],
  ['free_kicks', 'Bolas Paradas'],
  ['bravery', 'Bravura'],
  ['heading', 'Cabeceamento'],
  ['vision', 'Criatividade'],
  ['crossing', 'Cruzamentos'],
  ['decisions', 'Decisões'],
  ['tackling', 'Desarme'],
  ['determination', 'Determinação'],
  ['finishing', 'Finalização'],
  ['dribbling', 'Finta'],
  ['strength', 'Força'],
  ['jumping', 'Impulsão'],
  ['work_rate', 'Índice Trabalho'],
  ['leadership', 'Influência'],
  ['flair', 'Instinto'],
  ['handling', 'Jogo Mãos'],
  ['marking', 'Marcação'],
  ['passing', 'Passe'],
  ['positioning', 'Posicionamento'],
  ['reflexes', 'Reflexos'],
  ['long_shots', 'Remates Longe'],
  ['stamina', 'Resistência'],
  ['off_the_ball', 'Sem Bola'],
  ['technique', 'Técnica'],
  ['teamwork', 'Trabalho Equipa'],
  ['pace', 'Velocidade'],
]

/** A cor de um valor (1–20) como no perfil do CM: vermelho, laranja, amarelo, cinza. */
export function valueColor(v: number) {
  if (v >= 16) return 'text-[#ff3b3b]'
  if (v >= 11) return 'text-cm-orange'
  if (v >= 6) return 'text-[#f5e663]'
  return 'text-[#8f8fa0]'
}
