# NextCM — guia do logo (Fita)

O N com relevo, a partir do desenho do utilizador. O relevo sobe para a esquerda a 45°, paralelo à
diagonal: o verde só aparece nas hastes, como uma fita dobrada. O N é a primeira letra do nome, por
isso o ícone e o lockup horizontal são a mesma peça.

Tudo é gerado por `_build.py` (letras desenhadas à mão, sem fontes). Mude lá e corra
`python docs/logo/n-relevo/kit/_build.py`.

## Construção

- N numa caixa 120 × 160; hastes de 32; diagonal a 45° de canto a canto, bordas paralelas.
- Relevo: vetor (−14, −14) nas versões normais; (−20, −20) na tela inicial; (−22, −22) no corte
  pequeno (16–48 px), para o verde não sumir.
- "extCM": mesma altura de maiúscula (160), x-height 120, hastes de 32, cortes retos e cantos de fora
  arredondados. O N fica na mesma linha de base.

## Peças

| Ficheiro | Uso |
|---|---|
| `icone.svg`, `web/icone-*.png` | Ícone da app e do jogo (azulejo roxo), 64 px para cima |
| `favicon.svg`, `web/favicon.ico`, `web/favicon-*.png` | 16–48 px (corte pequeno) |
| `simbolo-escuro.svg` / `simbolo-claro.svg` | Só o N, sem azulejo (face branca / face roxa) |
| `simbolo-pequeno.svg` | Só o N para 16–48 px, fundo escuro |
| `horizontal-escuro.svg` / `horizontal-claro.svg` | NextCM completo |
| `horizontal-preto/branco.svg`, `simbolo-preto/branco/roxo.svg` | Uma cor: o N fica chapado, sem relevo |
| `tela-inicial.svg` (+ `-com-fundo`) | Versão grande: relevo mais fundo e a data 01/02 · 25/26 |
| `apresentacao.html`, `slides/` | Prancha com o logo aplicado |

## Cores

| Nome | HEX | RGB | Uso |
|---|---|---|---|
| Roxo | `#4F0BB4` | 79 11 180 | Azulejo do ícone; face do N e "CM" em fundo claro |
| Verde | `#1DD679` | 29 214 121 | Relevo; "CM" em fundo escuro |
| Escuro | `#1D0865` | 29 8 101 | Fundos escuros; "ext" em fundo claro |
| Branco | `#FFFFFF` | 255 255 255 | Face do N e "ext" em fundo escuro |

Contraste: branco sobre roxo 10,3:1; branco sobre escuro 16,5:1; verde sobre escuro 8,6:1.
**Verde sobre branco não passa (1,9:1)**: em fundo claro, o verde só no relevo, nunca no texto.

## Regras

- **Espaço livre:** à volta, pelo menos a largura de uma haste do N.
- **Tamanho mínimo:** ícone normal a partir de 64 px; abaixo disso, o favicon / corte pequeno.
  Horizontal a partir de 120 px de largura.
- **Não fazer:** mudar a direção ou a profundidade do relevo fora dos três vetores acima; pôr sombra,
  gradiente ou contorno; texto verde em fundo claro; trocar as letras por uma fonte; esticar.
- **Uma cor:** o relevo sai e o N fica chapado (as faces não se separam numa cor só).

## Notas

- Ainda não aplicado no jogo: os ícones do Tauri (`game/src-tauri/icons`) e a tela inicial continuam
  com o logo antigo. Os PNG de `web/` cobrem os tamanhos do Tauri (32, 128, 256…).
- "CM" remete para o Championship Manager, marca registada: vale uma busca de marcas antes de publicar.
