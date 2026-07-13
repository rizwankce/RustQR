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

1. **WP-012 dense multi-QR recall:** after rebuilding the release CLI,
   controlled `dense_50` is 43/50 (86%) in 394.62 ms with zero false
   positives, duplicates, or timeouts. The latency leg is met, but recall must
   reach 45/50. The fresh route audit identifies five Otsu-only payloads
   (`011`, `014`, `039`, `043`, `046`) and two absent from both routes
   (`021`, `047`). Direct Otsu is too slow as a global substitute; any hybrid
   must choose non-duplicate candidates generically and stay under 500 ms.
   A residual-Otsu proposal with eight reserved decode attempts regressed to
   40/50 in 911.42 ms and was reverted; do not repeat that design.
2. **WP-011 geometry/sampling:** expiry now prevents scheduling new image-wide
   passes, but scans already in flight cannot be interrupted. Continue only
   with bounded category-specific geometry or cancellable-stage evidence.
3. **WP-009 safety:** synthetic baseline is present; acceptance requires a
   licensed, representative negative corpus and explicit false-positive
   budget. `docs/wp009_adversarial.md` now defines the required provenance and
   manifest fields; the local BoofCV images are positive-only and cannot be
   repurposed as negative evidence.
4. **WP-005/WP-013 comparisons:** pinned standalone adapters for
   `main@5b9b41e` and `scratch_from_scratch_rebuild@295b97c` now live in
   `scripts/wp005_adapters/`. A detached main-only geometry projection patch
   now shows 17/17 monitor hits without fabricating boxes. Both adapters now
   use a shared explicit category allow-list and deterministic shard offsets.
   A complete local seven-category comparison now covers 157 matching images
   and 727 labels: rebuild is 74/727 versus main 36/727 (one main false
   positive, neither has duplicates/timeouts). It is diagnostic timing only;
   the matched Fast Benchmark Actions comparison and pinned competitor runners
   remain open.
5. **WP-010:** the bounded contour supplement improves `lots` finder/group
   recall to 102/420 and 87/420, but 215 labels still have no contained
   proposal. Preserve the capped primary/ROI/contour staging and do not relax
   thresholds without evidence carried through the evaluator.
6. **WP-011/WP-007/WP-014/WP-015:** use their explicit acceptance gaps in
   `TODO.md`; do not upgrade their statuses based on component-only tests.
   `format_extracted` is now a real strict-BCH observation count with distance
   buckets; RS candidate/block attempts, successes, and failures are separate
   from accepted decodes. Collect matched category traces before attempting a
   recovery change or attributing a failure to sampling.
   Fresh traces place bright-spots after exact format validation but before RS,
   while glare/high-version do not reach decoder evidence at all.

Run targeted checks while iterating, then record exact commands, metrics, and
remaining work in `TODO.md` before handoff. Full benchmark comparisons belong
on GitHub Actions when the packet explicitly requires them.
