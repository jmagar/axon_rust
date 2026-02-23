#!/usr/bin/env python3
"""Fail CI/pre-commit when newly changed files/functions become monolithic.

The check is ratcheting by default: only changed files and changed Rust functions
are enforced so existing legacy monoliths do not block progress.
"""

from __future__ import annotations

import argparse
import fnmatch
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
ALLOWLIST_FILE = REPO_ROOT / ".monolith-allowlist"

DEFAULT_FILE_MAX_LINES = 500
DEFAULT_FUNCTION_WARN_LINES = 80
DEFAULT_FUNCTION_MAX_LINES = 120

CHECK_EXTENSIONS = {".rs"}
EXCLUDED_GLOBS = [
    "config/**",
    "**/config/**",
    "**/config.rs",
    "tests/**",
    "**/tests/**",
    "**/*_test.*",
    "**/*_tests.*",
    "**/*.test.*",
    "**/*.spec.*",
    "benches/**",
]

HUNK_RE = re.compile(r"@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@")
FN_NAME_RE = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)")


@dataclass
class FunctionRange:
    name: str
    start: int
    end: int

    @property
    def length(self) -> int:
        return self.end - self.start + 1


def run_git(args: list[str]) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or f"git {' '.join(args)} failed")
    return result.stdout


def load_allowlist() -> set[str]:
    if not ALLOWLIST_FILE.exists():
        return set()

    allowed: set[str] = set()
    for raw in ALLOWLIST_FILE.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        allowed.add(line)
    return allowed


def is_excluded(path: str, allowlist: set[str]) -> bool:
    if path in allowlist:
        return True
    return any(fnmatch.fnmatch(path, pattern) for pattern in EXCLUDED_GLOBS)


def changed_files(base: str | None, head: str | None, staged: bool) -> list[str]:
    if staged:
        out = run_git(["diff", "--cached", "--name-only"])
    else:
        assert base is not None
        assert head is not None
        out = run_git(["diff", "--name-only", f"{base}...{head}"])
    files = []
    for line in out.splitlines():
        path = line.strip()
        if not path:
            continue
        full = REPO_ROOT / path
        if full.exists() and full.is_file():
            files.append(path)
    return files


def file_line_count(path: Path) -> int:
    """Count lines, excluding #[cfg(test)] mod blocks in .rs files."""
    lines = path.read_text(encoding="utf-8", errors="ignore").splitlines()
    if path.suffix != ".rs":
        return len(lines)

    count = 0
    pending_cfg_test = False
    skip_depth = 0

    for line in lines:
        stripped = line.strip()

        if skip_depth > 0:
            skip_depth += line.count("{") - line.count("}")
            continue

        if stripped.startswith("#[cfg(test)]"):
            pending_cfg_test = True
            continue

        if pending_cfg_test and stripped.startswith("mod ") and "{" in stripped:
            skip_depth = stripped.count("{") - stripped.count("}")
            pending_cfg_test = False
            continue

        if pending_cfg_test and stripped and not stripped.startswith("#"):
            pending_cfg_test = False

        count += 1

    return count


def parse_changed_line_numbers(
    base: str | None, head: str | None, path: str, staged: bool
) -> set[int]:
    if staged:
        patch = run_git(["diff", "--cached", "-U0", "--", path])
    else:
        assert base is not None
        assert head is not None
        patch = run_git(["diff", "-U0", f"{base}...{head}", "--", path])
    changed: set[int] = set()
    for line in patch.splitlines():
        match = HUNK_RE.match(line)
        if not match:
            continue
        start = int(match.group(1))
        count = int(match.group(2) or "1")
        if count <= 0:
            continue
        changed.update(range(start, start + count))
    return changed


def parse_rust_functions(path: Path) -> list[FunctionRange]:
    lines = path.read_text(encoding="utf-8", errors="ignore").splitlines()
    functions: list[FunctionRange] = []

    i = 0
    pending_cfg_test = False
    skip_test_module_depth = 0

    while i < len(lines):
        line_no = i + 1
        line = lines[i]
        stripped = line.strip()

        if skip_test_module_depth > 0:
            skip_test_module_depth += line.count("{") - line.count("}")
            i += 1
            continue

        if stripped.startswith("#[cfg(test)]"):
            pending_cfg_test = True
            i += 1
            continue

        if pending_cfg_test and stripped.startswith("mod ") and "{" in stripped:
            skip_test_module_depth = stripped.count("{") - stripped.count("}")
            pending_cfg_test = False
            i += 1
            continue

        if pending_cfg_test and stripped and not stripped.startswith("#"):
            pending_cfg_test = False

        if "fn " not in line:
            i += 1
            continue

        if stripped.startswith("//"):
            i += 1
            continue

        signature_start = i
        signature = line
        open_brace_line = None

        if "{" in line:
            open_brace_line = i
        elif ";" in line:
            i += 1
            continue
        else:
            j = i + 1
            while j < len(lines):
                signature += " " + lines[j].strip()
                if "{" in lines[j]:
                    open_brace_line = j
                    break
                if ";" in lines[j]:
                    break
                j += 1
            if open_brace_line is None:
                i = j + 1
                continue

        name_match = FN_NAME_RE.search(signature)
        fn_name = name_match.group(1) if name_match else "<anonymous>"

        depth = 0
        k = open_brace_line
        while k < len(lines):
            depth += lines[k].count("{")
            depth -= lines[k].count("}")
            if depth <= 0:
                functions.append(
                    FunctionRange(name=fn_name, start=signature_start + 1, end=k + 1)
                )
                break
            k += 1

        i = max(k + 1, i + 1)

    return functions


def validate_ref_exists(ref: str) -> None:
    try:
        run_git(["rev-parse", "--verify", ref])
    except RuntimeError as exc:
        raise RuntimeError(f"invalid git ref '{ref}': {exc}") from exc


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", help="Base git ref")
    parser.add_argument("--head", help="Head git ref")
    parser.add_argument(
        "--staged",
        action="store_true",
        help="Use staged changes from git index (for local pre-commit)",
    )
    parser.add_argument("--file-max-lines", type=int, default=DEFAULT_FILE_MAX_LINES)
    parser.add_argument(
        "--function-warn-lines", type=int, default=DEFAULT_FUNCTION_WARN_LINES
    )
    parser.add_argument("--function-max-lines", type=int, default=DEFAULT_FUNCTION_MAX_LINES)
    args = parser.parse_args()

    if not args.staged and (not args.base or not args.head):
        print(
            "provide --staged or both --base and --head",
            file=sys.stderr,
        )
        return 2

    try:
        if not args.staged:
            validate_ref_exists(args.base)
            validate_ref_exists(args.head)
        allowlist = load_allowlist()
        files = changed_files(args.base, args.head, args.staged)
    except RuntimeError as exc:
        print(f"monolith check setup failed: {exc}", file=sys.stderr)
        return 2

    violations: list[str] = []
    warnings: list[str] = []

    for path in files:
        if is_excluded(path, allowlist):
            continue

        full = REPO_ROOT / path
        if full.suffix not in CHECK_EXTENSIONS:
            continue

        line_count = file_line_count(full)
        if line_count > args.file_max_lines:
            violations.append(
                f"FILE {path}: {line_count} lines (limit {args.file_max_lines})"
            )

        if full.suffix != ".rs":
            continue

        changed_lines = parse_changed_line_numbers(
            args.base, args.head, path, args.staged
        )
        if not changed_lines:
            continue

        for fn in parse_rust_functions(full):
            # Only enforce when this function was touched in this change set.
            if not any(fn.start <= ln <= fn.end for ln in changed_lines):
                continue
            if fn.length > args.function_max_lines:
                violations.append(
                    "FUNCTION "
                    f"{path}:{fn.start} {fn.name}() is {fn.length} lines "
                    f"(limit {args.function_max_lines})"
                )
            elif fn.length > args.function_warn_lines:
                warnings.append(
                    "FUNCTION "
                    f"{path}:{fn.start} {fn.name}() is {fn.length} lines "
                    f"(warning {args.function_warn_lines}, limit {args.function_max_lines})"
                )

    if warnings:
        print("Monolith policy warnings:")
        for item in warnings:
            print(f"  - {item}")

    if violations:
        print("Monolith policy violations found:")
        for item in violations:
            print(f"  - {item}")
        print("\nAdd temporary exceptions to .monolith-allowlist only when necessary.")
        return 1

    print("Monolith policy check passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
