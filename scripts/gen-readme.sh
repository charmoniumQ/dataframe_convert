#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

replace_marker() {
    local target_file="$1" marker_start="$2" marker_end="$3" indentation="$4" file="$5"

  python3 - "$target_file" "$marker_start" "$marker_end" "$indentation" "$file" << 'PYEOF'

PYEOF
}

# ---- main.rs: metadata example output ----
echo "generating main.rs metadata example..."
INDENTED=$(echo "$META_OUTPUT" | sed 's/^/\/\/\/   /')
BLOCK='///   $ '"$META_CMD"'
///
'"$INDENTED"
replace_marker src/main.rs '// GEN-BEGIN-META' '// GEN-END-META' "$BLOCK"

# ---- README: main --help ----
echo "generating --help..."
OUTPUT=$("$BIN" --help 2>&1)
BLOCK='```sh
$ dataframe_convert --help
'"$OUTPUT"'
```'
replace_marker README.md '<!-- BEGIN HELP -->' '<!-- END HELP -->' "$BLOCK"

# ---- README: cat --help ----
echo "generating cat --help..."
OUTPUT=$("$BIN" cat --help 2>&1)
BLOCK='```sh
$ dataframe_convert cat --help
'"$OUTPUT"'
```'
replace_marker README.md '<!-- BEGIN HELP CAT -->' '<!-- END HELP CAT -->' "$BLOCK"

# ---- README: metadata --help ----
echo "generating metadata --help..."
OUTPUT=$("$BIN" metadata --help 2>&1)
BLOCK='```sh
$ dataframe_convert metadata --help
'"$OUTPUT"'
```'
replace_marker README.md '<!-- BEGIN HELP METADATA -->' '<!-- END HELP METADATA -->' "$BLOCK"

echo "done: updated README.md and src/main.rs"
