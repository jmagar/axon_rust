"""Execute the real structural hook against controlled checker failures."""

import os
from pathlib import Path
import shlex
import subprocess
import tempfile
import unittest

from check_lefthook_pre_commit_speed import parse_pre_commit_runs


class StructuralHookFailureTests(unittest.TestCase):
    def test_each_checker_failure_stops_both_dispatch_paths(self):
        repo = Path(__file__).resolve().parents[1]
        run = dict(parse_pre_commit_runs((repo / "lefthook.yml").read_text()))[
            "xtask-check"
        ]
        # The parser retains folded line breaks; shlex preserves the quoted
        # Bash program while reconstructing the wrapper's argument vector.
        argv = shlex.split(run)
        checks = [
            "check-no-mod-rs", "check-layering", "check-fetch-divergence",
            "check-claude-symlinks",
        ]
        for compiled in (True, False):
            for failing_index in range(len(checks)):
                with self.subTest(compiled=compiled, failing_index=failing_index):
                    with tempfile.TemporaryDirectory() as directory:
                        root = Path(directory)
                        (root / "scripts").symlink_to(repo / "scripts")
                        executable = root / (
                            "target/debug/xtask" if compiled else "bin/cargo"
                        )
                        executable.parent.mkdir(parents=True)
                        executable.write_text(
                            '#!/bin/bash\n'
                            'if [ "$1" = xtask ]; then shift; fi\n'
                            'echo "$1" >> "$CHECK_LOG"\n'
                            'if [ "$1" = "$FAIL_CHECK" ]; then exit 23; fi\n'
                        )
                        executable.chmod(0o755)
                        log = root / "checks.log"
                        result = subprocess.run(
                            argv, cwd=root, capture_output=True, text=True,
                            timeout=10,
                            env={**os.environ,
                                 "PATH": f"{root / 'bin'}:{os.environ['PATH']}",
                                 "CHECK_LOG": str(log),
                                 "FAIL_CHECK": checks[failing_index]},
                        )
                        self.assertEqual(result.returncode, 23, result.stderr)
                        self.assertEqual(
                            log.read_text().splitlines(), checks[:failing_index + 1]
                        )


if __name__ == "__main__":
    unittest.main()
