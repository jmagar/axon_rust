#!/usr/bin/env bash
# check_claude_symlinks.sh
# Warns if any CLAUDE.md lacks AGENTS.md and GEMINI.md symlinks in the same directory.
# Exits non-zero on missing or broken symlinks (blocks commit).

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
FAIL=0

while IFS= read -r -d '' claude_file; do
    dir="$(dirname "$claude_file")"
    rel_dir="${dir#"$REPO_ROOT/"}"
    [[ "$dir" == "$REPO_ROOT" ]] && rel_dir="."

    for target in AGENTS.md GEMINI.md; do
        link="$dir/$target"

        if [[ ! -e "$link" && ! -L "$link" ]]; then
            echo "[claude-symlinks] MISSING: $rel_dir/$target (should be a symlink to CLAUDE.md)"
            FAIL=1
        elif [[ ! -L "$link" ]]; then
            echo "[claude-symlinks] NOT A SYMLINK: $rel_dir/$target (must be: ln -sf CLAUDE.md $target)"
            FAIL=1
        else
            dest="$(readlink "$link")"
            if [[ "$dest" != "CLAUDE.md" ]]; then
                echo "[claude-symlinks] WRONG TARGET: $rel_dir/$target -> $dest (expected -> CLAUDE.md)"
                FAIL=1
            fi
        fi
    done
done < <(find "$REPO_ROOT" \
    -not -path "*/.git/*" \
    -not -path "*/node_modules/*" \
    -not -path "*/target/*" \
    -not -path "*/.cache/*" \
    -name "CLAUDE.md" \
    -print0)

if [[ $FAIL -ne 0 ]]; then
    echo ""
    echo "[claude-symlinks] Fix with: ln -sf CLAUDE.md AGENTS.md && ln -sf CLAUDE.md GEMINI.md"
    echo "[claude-symlinks] Run from each directory listed above."
    exit 1
fi

echo "[claude-symlinks] OK — all CLAUDE.md files have valid AGENTS.md + GEMINI.md symlinks"
exit 0
