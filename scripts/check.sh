#!/usr/bin/env bash
# Verificação completa do NextCM no Linux (o mesmo que o check.ps1): formatação e clippy do
# Rust, testes do core, TypeScript, oxlint e Prettier. Uma linha por passo e, nos que falham,
# só o fim da saída (os erros).
#
#   scripts/check.sh           # tudo
#   scripts/check.sh --quick   # sem os testes do core
#
# Os testes do core também regeneram os tipos TS em game/src/bindings (ts-rs), por isso
# correm antes do tsc.
cd "$(dirname "$0")/.."
quick=0
[ "$1" = "--quick" ] || [ "$1" = "-Quick" ] && quick=1
failed=()

step() {
    local name=$1
    shift
    local out
    if out=$("$@" 2>&1); then
        echo "ok      $name"
    else
        printf '\033[31mFALHOU  %s\033[0m\n' "$name"
        echo "$out" | tail -n 40
        failed+=("$name")
    fi
}

step 'cargo fmt' cargo fmt --all -- --check
step 'cargo clippy' cargo clippy --workspace --all-targets --quiet -- -D warnings
[ $quick = 1 ] || step 'cargo test (core)' cargo test -p nextcm-core --release --quiet
step 'tsc' bash -c 'cd game && npx tsc -b'
step 'oxlint' bash -c 'cd game && npx oxlint --deny-warnings'
step 'prettier' bash -c 'cd game && npx prettier --check src'

if [ ${#failed[@]} -gt 0 ]; then
    printf '\n\033[31m%d passo(s) falharam: %s\033[0m\n' ${#failed[@]} "$(IFS=,; echo "${failed[*]}")"
    exit 1
fi
echo
echo 'Tudo ok.'
