#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

python scripts/replace_block.py \
    src/main.rs \
    '///     $ dataframe_convert metadata data/sample.csv' \
    '///     # END' \
    '///     ' \
    <(cargo run -- metadata data/sample.csv)

cargo run -- --markdown-help metadata > README.md
