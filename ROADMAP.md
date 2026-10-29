# 🚀 NextCM - Documento Oficial de Arquitetura e Roadmap

Este documento serve como o **GDD (Game Design Document)** e **Plano de Engenharia** para recriarmos a base do Championship Manager 01/02 com tecnologia moderna. Atualizaremos este arquivo conforme avançamos.

---

## 🧱 0. Decisões de Stack (revisadas)

| Camada | Escolha | Por quê |
|---|---|---|
| UI | **React + TypeScript + Vite + Tailwind** | Já está montado em `game/`. Só exibe dados e envia comandos — **nenhuma regra de jogo no front**. |
| Lógica do jogo (core) | **Rust**, num crate próprio `nextcm-core` (biblioteca pura, sem dependência do Tauri) | Performance para simular ~140 mil jogadores por dia, tipagem forte para um modelo de dados grande. Por ser independente do Tauri, dá pra testar com `cargo test` e rodar temporadas inteiras por linha de comando, sem abrir a UI. |
| Desktop / ponte | **Tauri 2** (`game/src-tauri/`) | Entra na **Fase 2** (enciclopédia). Expõe o core para o React via `#[tauri::command]`. Os tipos TS são gerados a partir das structs Rust (`ts-rs` ou `specta`), então front e back nunca divergem. |
| Banco de dados | **SQLite** via `rusqlite` (no Rust) | O mundo importado vira um `world.sqlite`. Novo jogo = cópia desse arquivo; o save é o próprio SQLite. Durante a simulação o estado fica em memória (structs Rust) e é gravado no SQLite ao salvar. |
| Extração | **Python** (`scripts/`) | Lê os `.dat` originais e gera o `world.sqlite`. Roda uma vez por patch. (Pode ser portado pra Rust depois, se quisermos um importador dentro do app.) |

### Estrutura de pastas alvo
```
MeuCM/
├─ core/            # crate nextcm-core (lib): modelos, calendário, match engine, IA
│  └─ src/bin/sim.rs  # CLI: simula N temporadas e imprime tabelas (debug/benchmark)
├─ game/            # React + Vite
│  └─ src-tauri/    # crate do app: comandos Tauri que chamam o core
├─ scripts/         # extrator Python
└─ Cargo.toml       # workspace com core + game/src-tauri
```

### Regras de engenharia
- **Determinismo:** toda aleatoriedade passa por um RNG com *seed* (`rand_chacha::ChaCha8Rng`). Mesmo save + mesma seed = mesmo resultado. Isso torna bugs reproduzíveis e o match engine testável.
- **Simulação fora da thread da UI:** o "Continuar" roda numa thread/`tokio::task` do Rust e emite eventos de progresso (`app.emit`) para o React.
- **Dados estáticos ≠ estado dinâmico:** o que veio do `.dat` (nome, nascimento, nacionalidade) fica separado do que muda (atributos atuais, contrato, moral, clube).
- **Git desde já**, com os `.dat` e o `world.sqlite` no `.gitignore` (são pesados e o banco do patch CLZ não é nosso para redistribuir).

---

## 🎨 0.1 Diretrizes de UX — "tem que parecer o CM 01/02"

O objetivo é a experiência do CM 01/02: o mesmo design, a mesma velocidade e a mesma fluidez. Não queremos cara de dashboard moderno.

- **Visual (referência: CM 01/02 com o tema Coalizão do patch CLZ):** fundo escuro, **barra do topo** (logo NextCM, data em amarelo, clube, Continuar Jogo em verde), **coluna esquerda** cinza-azulada (◀ ▶, Treinador em rosa, Competições, Nações & Clubes, Procurar, Opções Jogo), **faixa de título colorida** com nome grande (cores da tela ou do clube), **abas escuras** (selecionada com borda amarela), **botões roxos** (Filtros, Atrás/Seguinte, Cancelar/Ok), tabela com cabeçalho roxo e **cores por coluna** (nome branco, clube magenta, nação amarelo, posição laranja), linha selecionada em vermelho, paginação "◀◀ ◀ Pg 1/N ▶ ▶▶" e **diálogo de filtros verde** com opções em caixa amarela. Nomes nas listas como "Sobrenome, Nome"; posições com as siglas em PT (GR, D, AL, MD, M, MA, A + lados D/E/C).
- **Sem enfeite:** sem animações, transições, cards ou espaçamento grande. A informação vem antes do estilo.
- **Toque moderno:** cantos levemente arredondados (botões, etiquetas, painéis) são bem-vindos, sem mudar a densidade nem a estrutura do CM. A barra do topo leva a marca NextCM (`docs/logo/`). Os menus em cascata (treinador, Competições) passaram de amarelos a verdes, com zebra, como os painéis novos.
- **Velocidade:** toda navegação deve parecer instantânea (< 100 ms). Consultas pesadas ficam no Rust, com índices no SQLite, e nunca com "loading" piscando.
- **Navegação:** voltar/avançar como no CM, com tudo clicável (jogador → clube → competição → nação).
- **Teclado:** setas para mover na lista, Enter para abrir, Alt+← / Alt+→ para voltar e avançar, Esc para limpar a busca.
- **Teclado:** começar a digitar em qualquer lugar já vai para a busca.
- **Data do jogo e "Continuar Jogo"** sempre visíveis no topo da coluna esquerda. Na Fase 2, o "Continuar Jogo" fica desabilitado.
- **Meta de desempenho medida:** a busca leva de 1 a 45 ms com 144 mil pessoas, graças a colunas pré-calculadas (`ability`, `birth_sort`, `positions`), `ANALYZE` e cache/mmap do SQLite.

## 🏗️ 1. Mapa de Arquitetura

```mermaid
flowchart TD
    subgraph extract["Extração (uma vez por patch)"]
        A[("Arquivos .dat do CM 01/02 + patch CLZ")]
        B(("scripts/ Python"))
    end

    subgraph app["NextCM App"]
        C[("world.sqlite / save.sqlite")]

        subgraph core["nextcm-core (Rust)"]
            E["Data Layer (rusqlite)"]
            D{"Match Engine"}
            F["Calendário + IA de Transferências"]
        end

        T["src-tauri: comandos + eventos"]

        subgraph ui["React + Tailwind"]
            G["Telas"]
            H["Busca de Jogadores"]
            I["Painel do Clube"]
        end
    end

    A -->|1. leitura binária| B
    B -->|2. gera| C
    C <-->|3. carrega / salva| E
    E <--> D
    E <--> F
    E <--> T
    T <-->|4. invoke / emit| G
    G --> H
    G --> I
```

---

## 🔬 2. Formato dos Dados

**Fonte:** os `.dat` de dentro do `CLZ Leagues Patch (DB 25-26) V5.4.rar`, extraídos em `data/raw/clz-25-26/`.
⚠️ A pasta `CLZ Leagues Patch (DB 25-26) V5.4/Data/` instalada no disco **não** tem o banco 25-26: ela foi sobrescrita pelo banco original de 2001 (Flamengo com Vampeta e Zagallo).

- **`index.dat`**: header de 8 bytes + entradas de 67 bytes (`nome[51]`, `id`, `quantidade`, `offset`, `versão`). É a fonte da verdade para contagens e offsets.
- **Strings**: `char[N]` terminadas em `\0`, Windows-1252.
- **Datas** (8 bytes): `short dia_do_ano` (base 0), `short ano`, `int bissexto`. Ano 1900 = desconhecido.
- **Anos com −24:** o patch guarda 2025 como 2001 (o CM 01/02 começa em 2001). O extrator soma 24 (`--year-offset`).
- **IDs**: −1/−2 = nenhum. Atributo 0 = não definido (o jogo gera na hora).
- **Atributos relativos:** 18 atributos técnicos (Antecipação, Cruzamentos, Decisões, Finta, Finalização, Jogo de Mãos, Cabeceamento, Remates Longe, Marcação, Sem Bola, 1x1, Passe, Penáltis, Posicionamento, Reflexos, Desarme, Lançamentos, Criatividade) são 1–20 *relativos* no banco. O jogo os converte em intrínsecos (−125 a 125) e mostra um valor que depende do CA (ver item 7 da sprint).
- IDs não colidem: seleções vêm depois dos clubes (11568+) e competições de seleções depois das de clubes (434+).

| Tabela (id no index) | Registro | Qtde | Observação |
|---|---|---|---|
| Pessoa (6) | 110 B | 144.423 | nomes, nascimento, nacionalidades, clube/cargo, contrato, salário, 8 atributos mentais, ponteiros |
| Non-player (9) | 68 B | 34.094 | CA/PA, reputação, 21 atributos de staff (ordem conferida no `cm0102.exe`; 8 são relativos, ver sprint item 9) |
| Player (10) | 70 B | 110.514 | CA/PA, reputação, 12 posições, 42 atributos 1–20, moral |
| Preferências (22) | 52 B | 144.423 | clubes/pessoas favoritos e odiados (não extraído) |
| Clube / Seleção (0/1) | 581 B | 11.568 / 462 | `division` = liga; `reserve_division` = **estadual** no patch CLZ |
| Competição (12/16) | 107 B | 434 / 24 | continente, nação, reputação |
| Histórico de carreira (17) | 17 B | 662.036 | tem duplicatas (o extrator remove) |

Os layouts completos estão em `scripts/extract.py`. Checagem: `python scripts/validate.py`.

---

## 🛤️ 3. Roadmap em 3 Fases

Primeiro os dados, depois a enciclopédia navegável, só então o jogo. **Nenhuma simulação nas Fases 1 e 2.**

### Fase 1 — Extrator ✅
**Objetivo:** transformar os `.dat` do patch CLZ num `world.sqlite` limpo e confiável.
- ✅ **1.1** `git init` + `.gitignore` (`*.dat`, `*.sqlite`, `node_modules`, `dist`, `target`).
- ✅ **1.2** Ler o `index.dat` (contagens e offsets de cada tabela).
- ✅ **1.3** Nomes: `first_names.dat`, `second_names.dat`, `common_names.dat`.
- ✅ **1.4** Pessoas: as 3 tabelas do `staff.dat` (staff, non-player, player).
- ✅ **1.5** Mundo: `club.dat`, `nation.dat`, `continent.dat`, `city.dat`, `stadium.dat`.
- ✅ **1.6** Competições: `club_comp.dat`, `nation_comp.dat` (+ históricos, se fácil).
- ✅ **1.7** Validação: checagem automática (0 órfãos) + conferência com o `cm0102ed-nick` usando um save de 2030, já que o editor não abre o banco puro. Batem os atributos fixos de jogador (Dirtiness, Handling, Flair, Consistency, Versatility...), os pés, 6 dos 8 mentais, o Flamengo (divisões, estadual, última posição, estádio, status e cores) e os atributos absolutos de staff. Ficha em `docs/conferencia-editor.md` (`python scripts/checklist.py`). Em aberto: Sportsmanship e Temperament aparecem sempre 2–5 pontos abaixo no save (pode ser o jogo mudando com o tempo), e o "Coaching Style" do editor (20) não bate com o nosso Coaching Technique (12).

**Pronto quando:** `python scripts/extract.py` gera o `world.sqlite` e as consultas de validação batem com o editor. ✅

### Fase 2 — Enciclopédia navegável (Tauri) (próxima)
**Objetivo:** abrir o app desktop e navegar por clubes, pessoas e competições. Só leitura.
- **2.1** Adicionar Tauri ao projeto Vite, montar o workspace Cargo (`core/` + `game/src-tauri/`), limpar o template.
- **2.2** `nextcm-core`: structs + consultas ao `world.sqlite` com `rusqlite`; tipos TS gerados (`ts-rs`/`specta`).
- **2.3** Comandos Tauri: `search_people`, `get_player`, `get_staff`, `get_club`, `get_squad`, `list_competitions`, `get_competition`, `get_nation`.
- **2.4** Telas:
  - **Clube:** dados, estádio, elenco e comissão técnica.
  - **Jogador / Staff:** perfil com atributos 1–20, posições, clube, nacionalidade, histórico.
  - **Competição:** clubes participantes (e campeões anteriores, se o histórico estiver extraído).
  - **Nação:** clubes e competições do país.
  - **Busca:** jogadores e staff com filtros (nome, idade, posição, nacionalidade, clube, atributos).
- **2.5** Navegação com links entre tudo (jogador → clube → competição → nação) e voltar/avançar.

**Pronto quando:** dá pra ir de "Brasil" até o perfil de qualquer jogador do Brasileirão clicando, e a busca responde em menos de ~200 ms.

### Fase 3 — Transformar em jogo
Dividida em marcos pequenos. Cada marco termina com algo jogável e testável, e as regras ficam no core em Rust (testes + CLI) antes de ganharem tela. Até o marco 3.5, só o Brasileirão Série A está "vivo"; as outras ligas entram no 3.6.

| Marco | Entrega | Pronto quando |
|---|---|---|
| ✅ **3.1** Novo jogo e tempo | Criar/gravar/carregar jogo, treinador no clube, "Continuar" avança a data, Notícias | Dá para começar um jogo no Palmeiras, avançar uns dias, gravar, fechar e carregar |
| ✅ **3.2** Série A jogável | Calendário (turno e returno), Match Engine v1 (força por setor → Poisson, RNG com seed), resultados, classificação real, Próximo/Último Jogo | Uma temporada inteira da Série A até o campeão |
| ✅ **3.3** O seu time | Escolher o onze, os suplentes e editar a táctica (formação, funções, instruções à equipa como no diálogo do CM), condição física, moral, lesões, suspensões | O onze e a táctica escolhidos mudam os resultados |
| ✅ **3.4** Dia de jogo | Match Engine v2 por eventos (remates, defesas, golos, cartões, substituições, minuto a minuto, mesma seed = mesmo jogo) e a tela da partida do CM: placar e relógio, relato em texto com velocidade ajustável, estatísticas, escalações; intervalo com mudança de táctica e substituições; resultados da jornada no fim | Ver um jogo do Palmeiras do início ao fim, mexer na equipa ao intervalo, e as médias do 3.2 (golos, vantagem em casa) continuam a bater na CLI |
| ✅ **3.5** Fim de época | Subidas/descidas, envelhecimento e evolução do CA, contratos, regens; CLI `sim` com 10 épocas | 10 épocas seguidas sem UI, com números plausíveis |
| ✅ **3.6** Mais competições | Época no ano civil; Séries C e D, Copa do Brasil, estaduais, Libertadores e Sul-Americana; regulamentos num arquivo de dados por competição. As ligas europeias passam para um marco seguinte | Calendário brasileiro completo numa época |
| ✅ **3.7** Mundo | Ligas de Inglaterra, Espanha, Itália, Alemanha, França, Portugal, Holanda, Argentina e Arábia Saudita (1ª e 2ª divisão), Liga dos Campeões e Liga Europa; épocas de agosto a maio ao lado do ano civil | Uma época europeia inteira com campeões, subidas e descidas e a final da Liga dos Campeões |
| ✅ **3.8** Mercado | Transferências e empréstimos (IA e treinador) nas janelas reais, renovações negociadas, finanças completas, orçamentos e despedimento | Clubes compram e vendem entre si com números plausíveis; o treinador compra, vende, renova e vê as finanças |
| ✅ **3.9** Personalidade | Os atributos ocultos que ainda não fazem nada (Ambição, Lealdade, Adaptabilidade, Versatilidade, Desportivismo, Temperamento) passam a contar no mercado, no motor e na moral | As médias do motor e do mercado continuam na CLI e cada atributo tem um efeito testado |
| ✅ **3.10** Olheiros e conhecimento | Atributos escondidos como no CM (o clube só vê o que conhece), observar jogadores, designar olheiros (clube, próximo adversário, jovens, país, região), listas Olheiro #1–#7, orçamento limitado para quem a direção não conhece, relatórios do médico e do preparador físico | Um jogador europeu desconhecido aparece com "-", é observado, o relatório chega e a direção passa a autorizar a proposta inteira |
| ✅ **3.11** Direcção e Federação | A tela Confiança da Direcção com o texto do CM; Reunião com a Direcção (expandir o estádio, investir nas camadas jovens, mais fundos para transferências e salários, mais tempo); Reunião com a Federação (queixas sobre a arbitragem, pedir adiamento de jogo) | Um pedido à direção é aceite ou recusado pela confiança e pelas finanças, e o efeito aparece (orçamento, estádio, juniores) |
| ✅ **3.12** Empregos e gestão do plantel | Informação de Empregos (procurar e aceitar clubes, despedimentos na IA), equipa de reservas, Disciplinar Jogador, elogiar e criticar jogadores na imprensa, conferências de imprensa antes e depois dos jogos (ou mandar o adjunto) | Despedido, o treinador encontra outro clube e continua o jogo; um castigo, um elogio ou uma resposta na conferência muda a moral pela personalidade |
| ✅ **3.13** Polimento | Notícias completas, férias, Auto Gravação, os ajustes pelos prints do 3.8 (menu Acção, proposta em tela inteira, Prémios e Cláusulas do contrato) | — |
| ✅ **3.14** Palestras | Discurso aos jogadores como no FM08: antes do jogo, ao intervalo, no fim e a quem entra; a reação de cada um pela personalidade e pela situação do jogo muda a moral e a motivação na partida | Uma palestra boa ou má muda a moral e o rendimento de forma visível, sem tirar o motor das médias na CLI |
| **3.15** Testes e ajustes | Jogar épocas inteiras no app e acertar o que aparecer: fluxos (Continuar Jogo, dia de jogo, mercado, mensagens), layouts e telas pelo CM 01/02, textos, números do motor e da economia, desempenho | Uma época inteira no Man Utd e no Palmeiras sem bugs nem telas fora do CM |
| **3.16** Plantel completo | O Plantel como no CM: menu do clube (principal, reservas, os outros clubes da divisão), Vêr (Tradicional, Contrato, Selecção, Estatísticas, Mais Dados, Atributos, Outros, Marcadores Penalties), Filtrar, siglas da coluna Inf, atalhos com ▸; reservas e clubes U21/B ligados ao clube principal | Ver o contrato, a forma e a nota média do plantel pelo Vêr, filtrar os médios disponíveis e saltar das reservas para o plantel principal |
| **3.17** Treino | A tela "<Clube> - Treinos": programas por grupo de jogadores (intensidade de Prep. Física, Táctica, Remate, Técnica e G-Redes, nova posição e novo lado), os treinadores, e o efeito no core (condição, evolução dos atributos, aprender uma posição) | Mudar o programa de um grupo muda a evolução numa época na CLI, e um jogador aprende uma posição nova |
| **3.18** Editor de jogos salvos | Abrir um save fechado a partir do menu inicial e editar jogadores, staff e clubes, com a validação no core | Mudar um atributo e um saldo, gravar, carregar o jogo e ver as mudanças |

#### Detalhe de cada marco

O detalhe (regras, números, decisões e o que ficou feito) está em `docs/roadmap/`, um ficheiro por marco ou ajuste. Abra só o que interessa.

- [Marco 3.1 — Novo jogo e tempo (detalhe)](docs/roadmap/3.01-novo-jogo-e-tempo.md)
- [Marco 3.2 — Série A jogável (detalhe)](docs/roadmap/3.02-serie-a-jogavel.md)
- [Marco 3.3 — O seu time (detalhe)](docs/roadmap/3.03-o-seu-time.md)
- [Marco 3.4 — Dia de jogo (detalhe)](docs/roadmap/3.04-dia-de-jogo.md)
- [Marco 3.5 — Fim de época (detalhe)](docs/roadmap/3.05-fim-de-epoca.md)
- [Marco 3.6 — Mais competições (detalhe)](docs/roadmap/3.06-mais-competicoes.md)
- [Marco 3.7 — Mundo: ligas estrangeiras e Europa (detalhe)](docs/roadmap/3.07-mundo.md)
- [Ajuste do motor: atributos mentais e peso da camisa (depois do 3.7)](docs/roadmap/3.07a-ajuste-do-motor.md)
- [Atributos gerados ao começar o jogo](docs/roadmap/3.07b-atributos-gerados.md)
- [Marco 3.8 — Mercado (detalhe)](docs/roadmap/3.08-mercado.md)
- [Números de camisola (depois do 3.8)](docs/roadmap/3.08a-numeros-de-camisola.md)
- [Marco 3.9 — Personalidade: atributos ocultos úteis](docs/roadmap/3.09-personalidade.md)
- [Ajustes da pesquisa e do mercado (depois do 3.9)](docs/roadmap/3.09a-ajustes-pesquisa-e-mercado.md)
- [Listas da IA, insatisfação e pedidos para sair (depois dos ajustes)](docs/roadmap/3.09b-insatisfacao-e-pedidos-para-sair.md)
- [Contratação como no CM (ajuste do mercado do treinador)](docs/roadmap/3.09c-contratacao-como-no-cm.md)
- [Correções e ajustes do teste de 24/10 (feito)](docs/roadmap/3.09d-teste-24-10.md)
- [Correções e ajustes do teste de 25/10 (por fazer)](docs/roadmap/3.09e-teste-25-10.md)
- [Marco 3.10 — Olheiros e conhecimento](docs/roadmap/3.10-olheiros-e-conhecimento.md)
- [Marco 3.11 — Direcção e Federação](docs/roadmap/3.11-direccao-e-federacao.md)
- [Marco 3.12 — Empregos, gestão do plantel e imprensa](docs/roadmap/3.12-empregos-e-gestao-do-plantel.md)
- [Marco 3.13 — Polimento: o fluxo do CM pela gravação](docs/roadmap/3.13-polimento.md)
- [Marco 3.14 — Palestras](docs/roadmap/3.14-palestras.md)
- [Marco 3.15 — Testes e ajustes](docs/roadmap/3.15-testes-e-ajustes.md)
- [Marco 3.16 — Plantel completo (pelos prints do CLZ 26, 28/10)](docs/roadmap/3.16-plantel-completo.md)
- [Marco 3.17 — Treino (pelos prints do CLZ 26, 28/10)](docs/roadmap/3.17-treino.md)
- [Marco 3.18 — Editor de jogos salvos (por detalhar)](docs/roadmap/3.18-editor-de-saves.md)

---

## 📋 4. Próxima Sprint (Fase 2)

1. [x] Ordem dos 21 atributos de staff conferida no `cm0102.exe` (`database_non_player_to_attribute_vector`, 0x53a8b0) e no editor. Os cargos 12–15 continuam sem conferência.
2. [x] Tauri adicionado ao `game/` (`npm run tauri dev`).
3. [x] Workspace Cargo com `core/` (nextcm-core) e `game/src-tauri/`.
4. [x] Comandos `search_people` e `list_nations` + tela de busca (nome sem acento, tipo, nação, posição, idade, ordenação, paginação).
5. [x] Visual Studio Build Tools instalado; `cargo test -p nextcm-core` (11 testes) e `npm run tauri dev` funcionando.
6. [x] Perfil de Jogador/Staff (abas Perfil, Contrato, Historial; cores do clube na faixa), aberto com duplo clique/Enter; navegação ◀ ▶ / Atrás-Seguinte / Alt+setas.
7. [x] Atributos "intrínsecos" convertidos para os valores que o CM mostra (`core/src/attributes.rs`; os valores do banco seguem em `rawAttributes`). O algoritmo foi lido do `cm0102.exe` 3.9.68: `intr = (10 × banco − CA) / 2`, duas normalizações "especialista × generalista" (0x538ec0, 0x538bc0) e a fórmula de exibição `x = (2·intr + CA)/20 + 10`, `(x²/10 + x)/3` (0x544380), que confere com o fórum champman0102 e o CMScoutIntrinsic. Yamal: Finalização 11 → 16 e Remates Longe 2 → 8. No jogo aparecem 18 e 9, porque o CM simula ~6 meses (treino, CA) antes da 1ª tela.
9. [x] Conversão dos atributos de staff (`displayed_staff_attributes` em `core/src/attributes.rs`). Ao criar o jogo, 8 atributos (Coaching GKs, Coaching, Judgement, Judging Potential, Man Handling, Motivating, Physiotherapy e Tactics) viram intrínsecos com `(10·v − CA)/2` (0x538ac6), sem as normalizações dos jogadores. A exibição (`non_player_attribute_rating`, 0x544410) usa a mesma fórmula dos jogadores, então o CA se cancela: `x = 10 + v/2`. Guardiola: Coaching 5 → 9. Os outros 13 são mostrados como estão no banco. Ainda falta conferir com um save tirado logo no início de um jogo novo.
8. [x] Tela de Clube: plantel em 2 colunas (Vêr jogadores/staff, Critério posição/nome/número/idade/habilidade) e Informação Geral; clube clicável na pesquisa, no perfil e nos rivais.
9. [x] Telas Competições (por continente), Competição (clubes e historial de campeões), Nações & Clubes e Nação (clubes por divisão, competições, seleção); botões da coluna ligados e links entre todas as telas.
10. [x] Fechar a Fase 2: o critério "do Brasil até qualquer jogador do Brasileirão só clicando" foi conferido (Nações & Clubes → Brazil → Palmeiras → Veiga → Brazil, e Competições → Sul Americanas → Brazil → Série A → clube → jogador). Ajustes: o botão **Competições** abre o menu amarelo em cascata do CM (Internacionais, Internacionais-Clubes, continentes → nações → competições; "Todas as Competições" abre a tela antiga). A nação no perfil virou link, as listas de nações e competições ficam em cache (sem "Nada para mostrar" piscando) e a faixa de título tem contorno preto e azul-ciano nas competições, como no CM.
11. [x] Telas que só dependem do banco:
    - **Tácticas** (clube → Tácticas): a formação preferida do treinador principal (`preferred_formation`, na ordem da lista do cm0102ed: 0 = não definida, 1 = 5-3-2 Líbero, 2 = 5-3-2, 3 = 4-4-2, 4 = 4-3-3, 5 = 4-2-4, 6 = 3-5-2, 7 = 4-1-2-1-2, 8 = 4-5-1, 9 = 3-4-3). O onze é escolhido em `core/src/tactics.rs` pelo melhor par função × jogador (nota de posição e lado × CA), mais 7 suplentes com um GR. Tem a lista e o campo da "Vista Geral". Editar a táctica fica para o marco 3.3.
    - **Historial do clube** (clube → Historial): títulos e vices por competição, tirados do historial das competições, e a posição da época anterior.
    - **Competição → Classificação:** tabela de pré-temporada, toda a zero e em ordem alfabética.
    - Reputação actual e mundial no perfil do staff.
12. [x] **Opções Jogo → Definições do Jogo** (diálogo verde do CM): Moeda (€, £, $, R$, câmbio aproximado do início de 25/26), Remunerações (semanais/mensais/anuais), Tipos de Letra dos títulos, Modo Ecrã (completo/em janela; no Tauri usa `setFullscreen`), cor da coluna esquerda (5 cores prontas ou uma escolhida, com relevos calculados a partir dela) e fundo (padrão, liso ou imagem própria reduzida para 1920 px, com escurecimento). Fica guardado no `localStorage` (`game/src/settings.ts`). Som e Auto Gravação aparecem desabilitados até a Fase 3.

> **Windows:** o Rust precisa do linker do MSVC. No Git Bash, o `link` do próprio Git atrapalha; rode `cargo` pelo PowerShell.
> **UI no navegador (sem Tauri):** `cargo run -p nextcm-core --example dev_bridge` (porta 1421) + `npm run dev` em `game/`. O Vite repassa `/invoke/*` para a ponte, que expõe os mesmos comandos. Só para desenvolvimento.
> Tipos TS em `game/src/bindings/` são gerados pelo `cargo test -p nextcm-core` (ts-rs) — não editar à mão.
