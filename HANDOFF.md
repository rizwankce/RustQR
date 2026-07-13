# Active Handoff

Read `AGENTS.md`, this file, and the current status records in `TODO.md`
before editing. `TODO.md` is the canonical detailed queue.

## Current state (2026-07-13)

- WP-001 through WP-004, WP-006, and WP-008 are complete.
- WP-002 Fast Benchmark validation is complete; it is no longer a gate.
- The worktree should normally be clean apart from intentionally untracked
  Python `__pycache__` directories. Preserve any new user changes.
- The active branch is `codex/wp006-conformance`; recent dense, scheduler, and
  evaluator work is already committed and pushed.

## Highest-value open packets

1. **WP-012 dense multi-QR recall:** controlled `dense_50` is 43/50 (86%) in
   485.14 ms with zero false positives, duplicates, or timeouts. The latency
   leg is met, but recall must reach 45/50. Finder coverage is already 50/50;
   the measured gap is post-group brightness-route geometry/decode. Direct
   Otsu reaches 48 but takes about 690 ms, so do not enable it globally.
2. **WP-011 geometry/sampling:** expiry now prevents scheduling new image-wide
   passes, but scans already in flight cannot be interrupted. Continue only
   with bounded category-specific geometry or cancellable-stage evidence.
3. **WP-009 safety:** synthetic baseline is present; acceptance requires a
   licensed, representative negative corpus and explicit false-positive
   budget.
4. **WP-013 competitors:** harness is implemented; pinned adapter builds must
   be supplied locally before comparative claims are possible.
5. **WP-005/WP-007/WP-010/WP-014/WP-015:** use their explicit acceptance gaps
   in `TODO.md`; do not upgrade their statuses based on component-only tests.

Run targeted checks while iterating, then record exact commands, metrics, and
remaining work in `TODO.md` before handoff. Full benchmark comparisons belong
on GitHub Actions when the packet explicitly requires them.
