# Live Test Scripts

## Consolidated Script

- `scripts/live-test-all-commands.sh`
  - Single in-repo harness that runs the full flow end-to-end.

## Original Phase Scripts (from the live run)

- `scripts/live-test-run-all-commands-phase1.sh`
  - Original phase 1 harness (top-level + initial subcommand pass)
- `scripts/live-test-run-subcommands-phase2.sh`
  - Original phase 2 subcommand harness
- `scripts/live-test-run-phase3-remaining.sh`
  - Original phase 3 completion harness

## Output Locations

Default consolidated output location:

- `.cache/live-test/<timestamp>/report.tsv`
- `.cache/live-test/<timestamp>/summary.txt`
- `.cache/live-test/<timestamp>/logs/*`

The original phase scripts preserve their original behavior and paths from the live run.
