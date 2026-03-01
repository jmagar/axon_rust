#!/usr/bin/env python3
"""PostToolUse hook: warn when deny.toml or .cargo/audit.toml is edited without the other.

Reads the edited file path from stdin JSON (Claude Code hook protocol).
Fires when either advisory config file is edited.
Checks whether the counterpart file has also been modified in the current
git working tree (staged or unstaged). If not, prints a reminder.

deny.toml is the canonical source of truth — .cargo/audit.toml must stay aligned.

Exits 0 always (reminder only — never blocks).
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]

PAIRS = {
    "deny.toml": ".cargo/audit.toml",
    "audit.toml": "deny.toml",
}


def is_modified(path: str) -> bool:
    """Return True if the file has any git modification (staged or unstaged)."""
    try:
        result = subprocess.run(
            ["git", "status", "--porcelain", path],
            capture_output=True,
            text=True,
            timeout=5,
            cwd=REPO_ROOT,
        )
        return bool(result.stdout.strip())
    except (subprocess.TimeoutExpired, FileNotFoundError):
        return True  # can't check → assume fine


def main() -> int:
    try:
        data = json.load(sys.stdin)
        file_path: str = data.get("tool_input", {}).get("file_path", "")
    except (json.JSONDecodeError, KeyError):
        return 0

    basename = Path(file_path).name
    counterpart = PAIRS.get(basename)
    if not counterpart:
        return 0

    if not is_modified(counterpart):
        canonical = "deny.toml" if basename == "audit.toml" else "deny.toml"
        print(
            f"[deny-audit-sync] Edited {file_path} but {counterpart} is unchanged — "
            f"both must stay aligned. {canonical} is canonical; mirror any "
            f"`ignore = [...]` changes into {counterpart} too."
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
