# NextCM — guia do logo (Emblema)

O emblema redondo de um clube antigo: uma moeda roxa com o N, e a estrela de campeão por cima.
Tudo é gerado por `_build.py` (letras desenhadas à mão, sem fontes): mude lá e rode
`python docs/logo/emblema/_build.py`.

## Peças

| Ficheiro | Uso |
|---|---|
| `simbolo.svg` | O símbolo principal (estrela + moeda). App, jogo, redes. |
| `simbolo-pequeno.svg` | Corte para 24–48 px: sem o anel verde, traços mais grossos. |
| `favicon.svg`, `web/favicon.ico` | 16–48 px: só a moeda, sem a estrela. |
| `icone-app.svg`, `web/icone-*.png` | Ícone da app (azulejo escuro). Para 32 px ou menos, `icone-app-pequeno.svg`. |
| `horizontal.svg` / `-negativo` | Símbolo + NextCM lado a lado (fundo claro / escuro). |
| `empilhado.svg` / `-negativo` | Símbolo por cima, NextCM por baixo. |
| `wordmark.svg` / `-negativo` | Só o NextCM, quando o símbolo já está por perto. |
| `emblema-completo.svg` | O emblema grande, com NEXTCM e 01/02 · 25/26 no anel e três estrelas. Só a partir de ~200 px. |
| `tela-inicial.svg` | Emblema completo + NextCM com sombra: a versão chamativa da tela inicial. |
| `simbolo-preto/branco/roxo.svg` | Uma cor (furos verdadeiros): carimbo, bordado, gravação. |
| `apresentacao.html`, `slides/` | Prancha com o logo aplicado. |

## Cores

| Nome | HEX | RGB | Uso |
|---|---|---|---|
| Roxo CM | `#4A1FC9` | 74 31 201 | A moeda, o "CM" em fundo claro |
| Verde gramado | `#1ED67A` | 30 214 122 | A estrela, o anel interior, o "CM" em fundo escuro |
| Sombra | `#1E0866` | 30 8 102 | O centro da moeda, a sombra do NextCM na tela inicial |
| Fundo | `#140A2E` | 20 10 46 | O azulejo do ícone, fundos escuros, o "Next" em fundo claro |
| Branco | `#FFFFFF` | 255 255 255 | O N, o anel exterior, o "Next" em fundo escuro |

## Regras

- **Espaço livre:** à volta do logo, pelo menos a altura da estrela.
- **Tamanho mínimo:** símbolo 24 px (abaixo disso use o favicon); horizontal 120 px de largura;
  emblema completo 200 px.
- **Fundos:** o símbolo colorido vai no branco, no creme e nos escuros da paleta. Em fotos ou em
  cores fora da paleta, use a versão branca ou preta.
- **Tela inicial:** o emblema completo com o NextCM ao lado, em fundo `#140A2E`. A sombra do
  NextCM só existe nessa versão.
- **Não fazer:** esticar, rodar ou mudar as cores; tirar a estrela do símbolo grande; pôr a
  versão com sombra em tamanhos pequenos; escrever texto no anel do símbolo pequeno; trocar o N
  por uma fonte.

## Notas

- O "NextCM" é desenhado à mão (hastes de 34, cantos de fora arredondados, de dentro retos,
  inclinado 12°), por isso não depende da licença de nenhuma fonte.
- O nome "CM" remete para o Championship Manager, que é marca registada; e "Football Manager",
  que aparecia numa das propostas antigas, é da SEGA. Vale fazer uma busca de marcas antes de
  publicar.
- Ainda não foi aplicado no jogo: os ícones do Tauri (`game/src-tauri/icons`) e a tela inicial
  continuam com o logo antigo.
