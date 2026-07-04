#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

python scripts/replace_block.py \
    src/main.rs \
    '///     $ dataframe_convert metadata data/sample.csv' \
    '///     # END' \
    '///     ' \
    <(cargo run -- metadata data/sample.csv)

python scripts/replace_block.py \
    README.md \
    '    $ dataframe_convert --help' \
    '    # END' \
    '    ' \
    <(cargo run -- --help)

python scripts/replace_block.py \
    README.md \
    '    $ dataframe_convert cat --help' \
    '    # END' \
    '    ' \
    <(cargo run -- cat --help)

python scripts/replace_block.py \
    README.md \
    '    $ dataframe_convert metadata --help' \
    '    # END' \
    '    ' \
    <(cargo run -- metadata --help)
