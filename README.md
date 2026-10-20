<div align="center">

# ⚽ NextCM

**O Championship Manager 01/02 que a gente ama, reconstruído com tecnologia moderna.**

O mesmo visual, a mesma velocidade, a mesma fluidez — com o banco de dados atualizado da época 2025/26.

![Rust](https://img.shields.io/badge/Rust-core-B7410E?logo=rust&logoColor=white)
![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)
![React](https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black)
![TypeScript](https://img.shields.io/badge/TypeScript-5A2A99?logo=typescript&logoColor=white)
![SQLite](https://img.shields.io/badge/SQLite-003B57?logo=sqlite&logoColor=white)
![Status](https://img.shields.io/badge/status-Fase%203%20·%20jogável-ffff40)

</div>

---

## 📖 Sobre

O **NextCM** recria a experiência do **Championship Manager 01/02** (com o tema *Coalizão* do patch CLZ) do zero: coluna esquerda cinza-azulada, faixas de título com as cores do clube, botões roxos, menus amarelos em cascata, tudo instantâneo. Sem cara de dashboard moderno: **a informação vem antes do estilo**.

O mundo do jogo vem dos arquivos `.dat` originais do CM 01/02 com o **CLZ Leagues Patch (DB 25-26)**: mais de **144 mil pessoas**, 11 mil clubes e 400 competições, extraídos para um SQLite. As regras do jogo vivem num núcleo em **Rust** determinístico — a mesma seed dá sempre a mesma época.

## ✨ O que já dá para fazer

### 🔎 Enciclopédia
- **Pesquisa de jogadores e staff** em milissegundos, com filtros (nome sem acento, posição, idade, nação, atributos) e o diálogo verde de filtros do CM.
- **Perfil** com os atributos 1–20 exatamente como o CM mostra (a conversão foi lida do próprio `cm0102.exe`), contrato, historial, lesões e castigos.
- **Clubes, competições e nações** ligados entre si: do Brasil até qualquer jogador do Brasileirão só clicando. Menu **Competições** em cascata, historial de campeões, títulos de cada clube.

### 🎮 Jogo (Série A do Brasileirão)
- **Novo jogo, gravar, carregar e apagar** — o save é um SQLite; saves antigos são migrados sozinhos.
- **Época inteira da Série A**: calendário de turno e returno, classificação real, Próximo/Último Jogo com local, tempo e notícias de cada lado.
- **Dia de jogo** como no CM: *Jogos de Hoje* → a partida ao vivo → *Resultados de Hoje* → caixa de mensagens.
- **Partida ao vivo** com motor por lances: relato minuto a minuto com velocidade ajustável, remates, defesas, golos com marcadores e assistências, cartões, lesões, estatísticas, notas dos jogadores e os outros jogos do dia; pausa no intervalo para **substituições** e **Instruções à Equipa**.
- **O seu time**: tácticas editáveis arrastando jogadores (na lista, no campo ou no plantel), posições livres no campo, setas de movimento com o botão direito, banco de 9, *Instruções à Equipa* e o adjunto que escolhe a equipa por você.
- **Condição física, moral, lesões, cartões e suspensões**, com notícias na caixa de mensagens.

### 🎨 Personalização
- Moeda (€, £, $, R$), salários semanais/mensais/anuais, fonte dos títulos, ecrã completo.
- **Cor da coluna lateral** e **fundo personalizado** com a sua própria imagem.

## 🏗️ Arquitetura

```
MeuCM/
├─ core/            # nextcm-core (Rust): dados, calendário, match engine, tácticas, save
│  ├─ src/live.rs         # match engine por lances (relato, cartões, substituições)
│  ├─ src/bin/sim.rs      # CLI: simula épocas inteiras sem interface
│  └─ examples/dev_bridge.rs  # ponte HTTP para abrir a UI no navegador em dev
├─ game/            # interface React + Vite + Tailwind
│  └─ src-tauri/    # app desktop Tauri 2 (comandos que chamam o core)
├─ scripts/         # extrator Python dos .dat → data/world.sqlite
└─ ROADMAP.md       # plano completo, decisões e estado de cada marco
```

| Camada | Tecnologia | Papel |
|---|---|---|
| Regras do jogo | **Rust** (`nextcm-core`) | Simulação, match engine com RNG seedado, escolha de equipa, save. Testável com `cargo test` e pela CLI, sem abrir a UI. |
| Desktop | **Tauri 2** | Expõe o core para a interface; os tipos TypeScript são gerados das structs Rust (`ts-rs`). |
| Interface | **React + TypeScript + Tailwind** | Só mostra dados e envia comandos — nenhuma regra de jogo no front. |
| Dados | **SQLite** | `world.sqlite` (o mundo) e um SQLite por save. |
| Extração | **Python** | Lê os `.dat` do CM 01/02 uma vez por patch. |

## 🚀 Como rodar

### Pré-requisitos
- [Rust](https://rustup.rs) (no Windows, com o linker do MSVC — *Visual Studio Build Tools*)
- [Node.js](https://nodejs.org) 20+
- Python 3
- Os arquivos `.dat` do **CLZ Leagues Patch (DB 25-26)** para o CM 01/02 (não estão no repositório — veja o aviso abaixo)

### 1. Gerar o mundo
Copie os `.dat` do patch para `data/raw/clz-25-26/` e rode:

```bash
python scripts/extract.py
```

Isso gera `data/world.sqlite`. Para conferir o resultado: `python scripts/validate.py`.

### 2. Abrir o jogo

```bash
cd game
npm install
npm run tauri dev
```

> **Windows:** rode o `cargo`/`tauri` pelo PowerShell — o `link` do Git Bash atrapalha o linker do MSVC.

### Desenvolvimento no navegador
Sem abrir o app desktop: suba a ponte de dev e o Vite (o Vite repassa `/invoke` para a ponte).

```bash
cargo run -p nextcm-core --example dev_bridge
npm --prefix game run dev
```

### Testes e simulação

```bash
powershell -File scripts/check.ps1                          # tudo: fmt, clippy, testes, tsc e oxlint
cargo test -p nextcm-core                                   # testes do core
cargo run -p nextcm-core --release --bin sim -- 7568 epoca 1 20   # 20 épocas da Série A
```

A CLI mostra a classificação, golos por jogo, vitórias em casa, remates, cartões, substituições e lesões por equipa, e a média de pontos e títulos de cada clube — é assim que o motor é calibrado.

## 🗺️ Roadmap

| | Marco | |
|---|---|---|
| ✅ | **Fase 1** | Extrator dos `.dat` → SQLite |
| ✅ | **Fase 2** | Enciclopédia navegável no visual do CM |
| ✅ | **3.1** | Novo jogo, gravar/carregar, tempo e notícias |
| ✅ | **3.2** | Série A jogável (calendário, match engine v1, classificação) |
| ✅ | **3.3** | O seu time: tácticas, condição, moral, lesões, suspensões |
| ✅ | **3.4** | Dia de jogo: motor por lances e relato ao vivo |
| ⏳ | **3.5** | Fim de época: subidas e descidas, evolução, contratos, regens |
| ⏳ | **3.6** | Mais competições (Série B, Copa do Brasil, estaduais, Libertadores...) |
| ⏳ | **3.7** | Mercado de transferências com IA e finanças |
| ⏳ | **3.8** | Polimento: notícias completas, empregos, férias |

O detalhe de cada marco — decisões, regras e números — está no [ROADMAP.md](ROADMAP.md).

## ⚠️ Aviso

O NextCM é um **projeto de fã, sem fins lucrativos**, sem ligação com a Sports Interactive, a Eidos/Square Enix ou os autores do patch CLZ. *Championship Manager* é marca dos seus respetivos donos.

**Nenhum dado do jogo original ou do patch é distribuído aqui**: os `.dat`, o `world.sqlite` e os saves ficam fora do Git (ver `.gitignore`). Para jogar, é preciso ter os arquivos originais.

---

<div align="center">

Feito com ⚽ e saudade das noites em claro no CM 01/02.

</div>
