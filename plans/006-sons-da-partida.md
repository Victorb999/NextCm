# 006 — Sons sintetizados da partida

- **Status**: DONE
- **Commit**: c8786e4
- **Severity**: LOW
- **Category**: Missed opportunities
- **Estimated scope**: 1 ficheiro novo (`game/src/sound.ts`) e `MatchDay.tsx`

## Target

Web Audio, sem ficheiros (decisão do utilizador: sons sintetizados).

- **Torcida de fundo**: ruído rosa em loop (buffer de 2 s), passa-banda 500 Hz (Q 0.6) e passa-baixo 2,2 kHz,
  ganho 0.10 × volume, a ondular devagar (LFO 0.13 Hz, ±25%). Liga com o jogo a correr e desliga (0.6 s) no
  intervalo, no fim, ao sair da partida ou com o Som desligado.
- **Lance perigoso** (frase `goal` que não é a do golo, `save`, `miss`): a torcida sobe para 0.22 em 0.4 s e
  volta em 1,5 s.
- **Golo**: da casa, rugido (ruído 300–3000 Hz, ataque 0.15 s, pico 0.55, cai em 3,5 s); do visitante, um "ohh"
  (passa-baixo 600 Hz, 0.25, 1,2 s) e um rugido baixo (0.18).
- **Apito**: `start` e `half`, dois pulsos de 0.25 s (sine 2900 Hz e 3100 Hz com vibrato de 30 Hz); `end`, três
  (curto, curto, longo).
- O `AudioContext` só se cria depois de um gesto do utilizador.
- Respeita `settings.sound` e `settings.volume`.

## Boundaries

- Sons só na tela da partida ao vivo.

## Verification

- **Mechanical**: `powershell -File scripts/check.ps1 -Quick`.
- **Feel check**: ver uma partida em Média com som. A torcida fica baixa, o golo da casa destaca-se e há apitos no
  início, no intervalo e no fim.
