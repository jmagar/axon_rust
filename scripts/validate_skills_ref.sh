#!/usr/bin/env bash
set -euo pipefail

mapfile -t staged_skill_files < <(git diff --cached --name-only --diff-filter=ACMR | awk '/^skills\//')

if [ "${#staged_skill_files[@]}" -eq 0 ]; then
  echo "[skills-ref] no staged skill changes"
  exit 0
fi

if ! command -v skills-ref >/dev/null 2>&1; then
  echo "[skills-ref] 'skills-ref' is required to validate staged skills."
  echo "[skills-ref] install it, then retry commit."
  exit 1
fi

declare -A skill_dirs=()
for file in "${staged_skill_files[@]}"; do
  # Validate top-level skill directories under skills/<skill-name>/...
  if [[ "$file" =~ ^skills/([^/]+)/ ]]; then
    skill_dir="skills/${BASH_REMATCH[1]}"
    if [ -f "$skill_dir/SKILL.md" ]; then
      skill_dirs["$skill_dir"]=1
    fi
  fi
done

if [ "${#skill_dirs[@]}" -eq 0 ]; then
  echo "[skills-ref] no staged top-level skill directories with SKILL.md"
  exit 0
fi

echo "[skills-ref] validating ${#skill_dirs[@]} skill(s)"
for skill_dir in "${!skill_dirs[@]}"; do
  echo "[skills-ref] validate $skill_dir"
  skills-ref validate "$skill_dir"
done

echo "[skills-ref] validation passed"
