"""Allowlisted failure metadata; never retain exception messages or source text."""
from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

ERROR_TYPES = frozenset({
    "unknown", "AssertionError", "RuntimeError", "ValueError", "TypeError",
    "KeyError", "IndexError", "OSError", "FileNotFoundError", "PermissionError",
    "ProcessLookupError", "TimeoutError", "TimeoutExpired", "CalledProcessError",
    "HTTPError", "URLError", "RemoteDisconnected", "IncompleteRead",
    "BrokenPipeError", "ConnectionResetError", "ConnectionRefusedError",
    "JSONDecodeError", "UnicodeDecodeError", "AcceptanceError", "SecurityError", "WireError",
})
PHASES = frozenset({
    "launcher", "doctor", "observability", "retrieval", "domains",
    "observe-cli", "observe-http-job", "observe-http-events", "observe-mcp",
    "observe-http-stats", "observe-oracles", "observe-report",
})
WIRE_FIELDS = frozenset({"wire_context", "rpc_code", "internal_context"})
FIELDS = frozenset({"domain", "error_type", "child_returncode", "traceback_file", "traceback_line"}) | WIRE_FIELDS
EXCEPTION_LINE = re.compile(r"^([A-Za-z_][A-Za-z0-9_.]*):(?: |$)")
FRAME_LINE = re.compile(r'^  File "([^"\r\n]+)", line ([0-9]+), in ')


def _domains(root: Path) -> set[str]:
    return set(PHASES) | {path.parent.name for path in
                         (root / "tests/e2e/scenarios").glob("*/hermetic_entry.py")}


def _location(root: Path, filename: object, line: object) -> str | None:
    if not isinstance(filename, str) or type(line) is not int or line < 1:
        return None
    root = root.resolve()
    try:
        path = (root / filename).resolve()
        relative = path.relative_to(root).as_posix()
        # Only repository-contained harness source locations are useful. In particular,
        # never retain arbitrary run-owned paths, config names, or messages.
        if path.suffix != ".py" or not (
            relative.startswith(("tests/e2e/", "scripts/e2e/"))
            or relative == "scripts/test-mcp-tasks-wire.py"
        ):
            return None
        if line > len(path.read_text().splitlines()):
            return None
    except (OSError, ValueError, RuntimeError, UnicodeError):
        return None
    return relative


def validate(root: Path, value: object) -> dict | None:
    """Revalidate metadata at the report boundary, including exact field types."""
    if not isinstance(value, dict) or not {"domain", "error_type"} <= value.keys() <= FIELDS:
        return None
    if not isinstance(value["domain"], str) or value["domain"] not in _domains(root):
        return None
    if not isinstance(value["error_type"], str) or value["error_type"] not in ERROR_TYPES:
        return None
    if WIRE_FIELDS & value.keys():
        if value["error_type"] != "WireError":
            return None
        if "wire_context" in value and value["wire_context"] not in ("initialize", "stdio_capabilities"):
            return None
        if "internal_context" in value and value["internal_context"] not in ("capabilities.context", "capabilities.doctor"):
            return None
        if "rpc_code" in value and (type(value["rpc_code"]) is not int or not -(2**31) <= value["rpc_code"] < 2**31):
            return None
    if "child_returncode" in value and (
        type(value["child_returncode"]) is not int or not -(2**31) <= value["child_returncode"] < 2**31
    ):
        return None
    if ("traceback_file" in value) != ("traceback_line" in value):
        return None
    if "traceback_file" in value:
        location = _location(root, value["traceback_file"], value["traceback_line"])
        if location is None or location != value["traceback_file"]:
            return None
    return dict(value)


def child_failure(root: Path, domain: str, child: subprocess.CompletedProcess[str]) -> dict:
    """Extract only class, exit status and the last valid repository frame."""
    value = {"domain": domain, "error_type": "unknown", "child_returncode": child.returncode}
    for line in reversed(child.stderr.splitlines()):
        match = EXCEPTION_LINE.match(line)
        if match:
            name = match[1].rsplit(".", 1)[-1]
            if name in ERROR_TYPES:
                value["error_type"] = name
            if name == "WireError":
                try:
                    metadata = json.loads(line[match.end():])
                except (ValueError, RecursionError):
                    metadata = None
                if isinstance(metadata, dict) and metadata.keys() <= WIRE_FIELDS:
                    candidate = {**value, **metadata}
                    if validate(root, candidate) is not None:
                        value = candidate
            # Do not mistake an earlier chained exception for the final error.
            break
    for line in reversed(child.stderr.splitlines()):
        match = FRAME_LINE.match(line)
        if match:
            number = int(match[2])
            location = _location(root, match[1], number)
            if location is not None:
                value.update(traceback_file=location, traceback_line=number)
                break
    safe = validate(root, value)
    if safe is None:
        raise ValueError("invalid failure diagnostic metadata")
    return safe


def exception_failure(root: Path, domain: str, error: BaseException) -> dict:
    """Read traceback identity only; never format the exception or frame locals."""
    name = type(error).__name__
    value = {"domain": domain, "error_type": name if name in ERROR_TYPES else "unknown"}
    frame = error.__traceback__
    while frame is not None:
        location = _location(root, frame.tb_frame.f_code.co_filename, frame.tb_lineno)
        if location is not None:
            value.update(traceback_file=location, traceback_line=frame.tb_lineno)
        frame = frame.tb_next
    safe = validate(root, value)
    if safe is None:
        raise ValueError("invalid failure diagnostic metadata")
    return safe
