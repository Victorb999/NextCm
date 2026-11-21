#!/usr/bin/env bash
# Abre o NextCM no navegador no Linux (o mesmo que o web.ps1): compila a ponte de dev (o core,
# em release para o Continuar Jogo ser rápido), liga-a na porta 1421 e corre o Vite (a
# interface) na 5173, que abre o navegador. Ctrl+C fecha os dois.
#
#   scripts/web.sh              # compila e abre
#   scripts/web.sh --no-build   # usa a ponte já compilada
#   scripts/web.sh --no-open    # não abre o navegador sozinho
#
# O jogo aberto vive na memória da ponte: fechá-la perde o que não foi gravado.
cd "$(dirname "$0")/.."
root=$(pwd)
bridge="$root/target/release/examples/dev_bridge"
build=1
open=1
for a in "$@"; do
    case $a in
        --no-build | -NoBuild) build=0 ;;
        --no-open | -NoOpen) open=0 ;;
    esac
done

port_in_use() { (echo >"/dev/tcp/127.0.0.1/$1") 2>/dev/null; }

if port_in_use 1421; then
    echo 'A porta 1421 já está ocupada (outra ponte de dev aberta?). Feche-a e tente de novo.' >&2
    exit 1
fi

if [ $build = 1 ]; then
    echo 'A compilar a ponte de dev (release)...'
    cargo build -p nextcm-core --release --example dev_bridge || exit $?
fi
if [ ! -x "$bridge" ]; then
    echo "Não encontrei $bridge. Corra sem --no-build." >&2
    exit 1
fi

if [ ! -d game/node_modules ]; then
    echo 'A instalar as dependências do front (npm install)...'
    npm --prefix game install || exit $?
fi

"$bridge" &
pid=$!
trap 'kill $pid 2>/dev/null; echo "Ponte de dev fechada."' EXIT

# Espera a ponte abrir a porta (lê o banco ao arrancar)
for _ in $(seq 60); do
    port_in_use 1421 && break
    kill -0 $pid 2>/dev/null || { echo 'A ponte de dev fechou ao arrancar.' >&2; exit 1; }
    sleep 0.5
done
printf '\033[32mPonte de dev na porta 1421. A abrir o NextCM em http://localhost:5173 ...\033[0m\n'
if [ $open = 1 ]; then npm --prefix game run dev -- --open; else npm --prefix game run dev; fi
