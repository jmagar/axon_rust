#!/usr/bin/env python3
"""Fail CI/pre-commit when newly changed files/functions become monolithic.

The check is ratcheting by default: only changed files are enforced so existing
legacy monoliths do not block progress.

- All changed files are checked against a file line limit.
- Rust files additionally get per-function size enforcement.
"""

from __future__ import annotations

import argparse
import ast
import fnmatch
import io
import re
import subprocess
import sys
import token as token_mod
import tokenize
from dataclasses import dataclass
from pathlib import Path


def _find_repo_root() -> Path:
    try:
        root = subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"],
            cwd=Path.cwd(),
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        return Path(root)
    except (subprocess.CalledProcessError, FileNotFoundError):
        return Path.cwd()

REPO_ROOT = _find_repo_root()
ALLOWLIST_FILE = REPO_ROOT / ".monolith-allowlist"

DEFAULT_FILE_MAX_LINES = 500
DEFAULT_FUNCTION_WARN_LINES = 80
DEFAULT_FUNCTION_MAX_LINES = 120

RUST_EXTENSIONS = {".rs"}
CHECKABLE_EXTENSIONS = {
    ".rs",
    ".py",
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
    ".go",
    ".java",
    ".kt",
    ".swift",
    ".cpp",
    ".c",
    ".h",
    ".sh",
    ".bash",
    ".zsh",
}
EXCLUDED_GLOBS = [
    "config/**",
    "**/config/**",
    "**/config.rs",
    "tests/**",
    "**/tests/**",
    "**/tests.rs",
    "**/*_test.*",
    "**/*_tests.*",
    "**/*.test.*",
    "**/*.spec.*",
    "benches/**",
]

HUNK_RE = re.compile(r"@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@")
FN_NAME_RE = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)")

SLASH_COMMENT_EXTENSIONS = {
    ".rs",
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
    ".go",
    ".java",
    ".kt",
    ".swift",
    ".cpp",
    ".c",
    ".h",
}

HASH_COMMENT_EXTENSIONS = {".py", ".sh", ".bash", ".zsh"}


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
    """Count lines, excluding Rust test modules in .rs files."""
    lines = path.read_text(encoding="utf-8", errors="ignore").splitlines()
    if path.suffix != ".rs":
        return count_effective_lines(lines, path.suffix)

    filtered_lines: list[str] = []
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

        # Also exclude one-line test module declarations:
        # #[cfg(test)]
        # mod tests;
        if pending_cfg_test and stripped.startswith("mod ") and stripped.endswith(";"):
            pending_cfg_test = False
            continue

        if pending_cfg_test and stripped and not stripped.startswith("#"):
            pending_cfg_test = False

        filtered_lines.append(line)

    return count_effective_lines(filtered_lines, ".rs")


def count_effective_lines(lines: list[str], suffix: str) -> int:
    """Count substantive lines, ignoring blanks and comment-only lines."""
    if suffix == ".py":
        return count_effective_python_lines("\n".join(lines))
    if suffix in SLASH_COMMENT_EXTENSIONS:
        return count_effective_c_like_lines(lines)
    if suffix in HASH_COMMENT_EXTENSIONS:
        return sum(1 for raw in lines if raw.strip() and not raw.lstrip().startswith("#"))
    return sum(1 for raw in lines if raw.strip())


def count_effective_c_like_lines(lines: list[str]) -> int:
    """Count non-empty, non-comment C-like lines with stateful comment stripping."""
    in_block_comment = False
    count = 0

    for raw in lines:
        i = 0
        line = raw
        out: list[str] = []
        in_single = False
        in_double = False
        escaped = False

        while i < len(line):
            ch = line[i]
            nxt = line[i + 1] if i + 1 < len(line) else ""

            if in_block_comment:
                if ch == "*" and nxt == "/":
                    in_block_comment = False
                    i += 2
                    continue
                i += 1
                continue

            if in_single:
                out.append(ch)
                if escaped:
                    escaped = False
                elif ch == "\\":
                    escaped = True
                elif ch == "'":
                    in_single = False
                i += 1
                continue

            if in_double:
                out.append(ch)
                if escaped:
                    escaped = False
                elif ch == "\\":
                    escaped = True
                elif ch == '"':
                    in_double = False
                i += 1
                continue

            if ch == "/" and nxt == "/":
                break
            if ch == "/" and nxt == "*":
                in_block_comment = True
                i += 2
                continue
            if ch == "'":
                in_single = True
                out.append(ch)
                i += 1
                continue
            if ch == '"':
                in_double = True
                out.append(ch)
                i += 1
                continue

            out.append(ch)
            i += 1

        if "".join(out).strip():
            count += 1

    return count


def _python_docstring_line_spans(text: str) -> set[int]:
    spans: set[int] = set()
    try:
        tree = ast.parse(text)
    except SyntaxError:
        return spans

    stack = [tree]
    while stack:
        node = stack.pop()
        body = getattr(node, "body", None)
        if isinstance(body, list) and body:
            first = body[0]
            if (
                isinstance(first, ast.Expr)
                and isinstance(first.value, ast.Constant)
                and isinstance(first.value.value, str)
            ):
                start = getattr(first, "lineno", None)
                end = getattr(first, "end_lineno", start)
                if isinstance(start, int) and isinstance(end, int):
                    spans.update(range(start, end + 1))
        for child in ast.iter_child_nodes(node):
            stack.append(child)

    return spans


def count_effective_python_lines(text: str) -> int:
    """Count substantive Python lines, excluding comments and docstrings."""
    docstring_lines = _python_docstring_line_spans(text)
    significant_lines: set[int] = set()
    stream = io.StringIO(text)

    try:
        tokens = tokenize.generate_tokens(stream.readline)
        for tok in tokens:
            tok_type = tok.type
            line_no = tok.start[0]
            if tok_type in {
                token_mod.INDENT,
                token_mod.DEDENT,
                token_mod.NEWLINE,
                tokenize.NL,
                token_mod.ENDMARKER,
                token_mod.COMMENT,
            }:
                continue
            if tok_type == token_mod.STRING and line_no in docstring_lines:
                continue
            significant_lines.add(line_no)
    except tokenize.TokenError:
        for idx, raw in enumerate(text.splitlines(), start=1):
            stripped = raw.strip()
            if stripped and not stripped.startswith("#"):
                significant_lines.add(idx)

    return len(significant_lines)


def is_text_file(path: Path) -> bool:
    """Best-effort binary detection: treat files with NUL bytes as binary."""
    try:
        chunk = path.read_bytes()[:8192]
    except OSError:
        return False
    return b"\x00" not in chunk


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

        if pending_cfg_test and stripped.startswith("mod ") and stripped.endswith(";"):
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


def normalize_file_arg(file_arg: str) -> str:
    path = Path(file_arg)
    if path.is_absolute():
        try:
            return str(path.resolve().relative_to(REPO_ROOT))
        except ValueError as exc:
            raise RuntimeError(f"--file path must be inside repo root: {REPO_ROOT}") from exc
    return file_arg


def _self_test() -> int:
    tests = []

    tests.append(
        (
            "c_like_comments",
            count_effective_c_like_lines(
                [
                    "let a = 1; // trailing",
                    "/* block start",
                    "still block */ let b = 2;",
                    "let s = \"http://x\"; // url",
                    "   ",
                    "// comment only",
                ]
            )
            == 3,
        )
    )

    tests.append(
        (
            "python_docstrings",
            count_effective_python_lines(
                '"""module docs"""\n\n'
                "def f():\n"
                '    """function docs"""\n'
                "    x = 1\n"
                "    return x\n"
            )
            == 3,
        )
    )

    tests.append(
        (
            "hash_comments",
            count_effective_lines(["# c1", "x=1", "   ", "echo hi # inline"], ".sh") == 2,
        )
    )

    tests.append(
        (
            "cfg_test_decl_excluded",
            file_line_count_from_text(
                "#[cfg(test)]\nmod tests;\nfn real() {\n 1\n}\n",
                ".rs",
            )
            == 3,
        )
    )

    tests.append(
        (
            "unknown_suffix_fallback",
            count_effective_lines(["", "a", "# x"], ".md") == 2,
        )
    )

    failed = [name for name, ok in tests if not ok]
    if failed:
        print("self-test failed:", ", ".join(failed), file=sys.stderr)
        return 1
    print("self-test passed")
    return 0


def file_line_count_from_text(text: str, suffix: str) -> int:
    lines = text.splitlines()
    if suffix != ".rs":
        return count_effective_lines(lines, suffix)

    filtered_lines: list[str] = []
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
        if pending_cfg_test and stripped.startswith("mod ") and stripped.endswith(";"):
            pending_cfg_test = False
            continue
        if pending_cfg_test and stripped and not stripped.startswith("#"):
            pending_cfg_test = False
        filtered_lines.append(line)
    return count_effective_lines(filtered_lines, ".rs")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", help="Base git ref")
    parser.add_argument("--head", help="Head git ref")
    parser.add_argument("--file", help="Check a single repo-relative or absolute file path")
    parser.add_argument(
        "--staged",
        action="store_true",
        help="Use staged changes from git index (for local pre-commit)",
    )
    parser.add_argument("--self-test", action="store_true", help="Run internal detector self-tests")
    parser.add_argument("--file-max-lines", type=int, default=DEFAULT_FILE_MAX_LINES)
    parser.add_argument(
        "--function-warn-lines", type=int, default=DEFAULT_FUNCTION_WARN_LINES
    )
    parser.add_argument("--function-max-lines", type=int, default=DEFAULT_FUNCTION_MAX_LINES)
    args = parser.parse_args()

    if args.self_test:
        return _self_test()

    if not args.file and not args.staged and (not args.base or not args.head):
        print(
            "provide --file, --staged, or both --base and --head",
            file=sys.stderr,
        )
        return 2

    try:
        if args.file:
            rel = normalize_file_arg(args.file)
            files = [rel]
        elif not args.staged:
            validate_ref_exists(args.base)
            validate_ref_exists(args.head)
            files = changed_files(args.base, args.head, args.staged)
        else:
            files = changed_files(args.base, args.head, args.staged)
        allowlist = load_allowlist()
    except RuntimeError as exc:
        print(f"monolith check setup failed: {exc}", file=sys.stderr)
        return 2

    violations: list[str] = []
    warnings: list[str] = []

    for path in files:
        if is_excluded(path, allowlist):
            continue

        full = REPO_ROOT / path
        if not is_text_file(full):
            continue
        if full.suffix not in CHECKABLE_EXTENSIONS:
            continue

        line_count = file_line_count(full)
        if line_count > args.file_max_lines:
            violations.append(
                f"FILE {path}: {line_count} lines (limit {args.file_max_lines})"
            )

        if full.suffix not in RUST_EXTENSIONS:
            continue

        rust_lines = full.read_text(encoding="utf-8", errors="ignore").splitlines()
        if args.file:
            changed_lines = set(range(1, len(rust_lines) + 1))
        else:
            changed_lines = parse_changed_line_numbers(
                args.base, args.head, path, args.staged
            )
            if not changed_lines:
                continue

        for fn in parse_rust_functions(full):
            # Only enforce when this function was touched in this change set.
            if not any(fn.start <= ln <= fn.end for ln in changed_lines):
                continue
            fn_len = count_effective_lines(rust_lines[fn.start - 1 : fn.end], ".rs")
            if fn_len > args.function_max_lines:
                violations.append(
                    "FUNCTION "
                    f"{path}:{fn.start} {fn.name}() is {fn_len} lines "
                    f"(limit {args.function_max_lines})"
                )
            elif fn_len > args.function_warn_lines:
                warnings.append(
                    "FUNCTION "
                    f"{path}:{fn.start} {fn.name}() is {fn_len} lines "
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
