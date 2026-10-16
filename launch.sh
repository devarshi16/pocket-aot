#!/usr/bin/env bash
set -euo pipefail
arcade_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
export POCKET_ARCADE_HOME="$arcade_dir"
cd "$arcade_dir"
exec cargo run --release --locked --bin pocket-arcade -- "$@"
