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

1. **WP-012 dense multi-QR recall:** dense_50 now clears its controlled gate:
   a bounded breadth-first regional schedule gives 48/50 (96%) in 97.09 ms,
   with zero false positives, duplicates, or timeouts; repeat samples were
   95.84–101.91 ms. It does not complete WP-012: the same ladder is 23/25 and
   63/100. A one-image realistic `lots` check is only 1/60 in 505.77 ms at
   `QR_MAX_DIM=800`; realistic dense scenes still need proposal-recall work.
   The route audit's
   five Otsu-only payloads (`011`, `014`, `039`, `043`, `046`) and two absent
   from both routes (`021`, `047`) remain useful diagnosis, but global or
   residual Otsu retries are not viable under the bounded request policy.
   A residual-Otsu proposal with eight reserved decode attempts regressed to
   40/50 in 911.42 ms and was reverted; do not repeat that design.
2. **WP-011 geometry/sampling:** expiry now prevents scheduling new image-wide
   passes, but scans already in flight cannot be interrupted. Continue only
   with bounded category-specific geometry or cancellable-stage evidence.
3. **WP-009 safety:** synthetic baseline is present; acceptance requires a
   licensed, representative negative corpus and explicit false-positive
   budget. A 47-image/12.5184 MP Apache-2.0 ZXing high-contrast negative
   slice is now vendored with per-asset hashes, provenance, and zero-timeout
   public-API evidence. A separate 17 Aztec/23 Data Matrix/7 Code128 ZXing
   slice has complete membership/format/hash provenance and now passes its
   strict five-second release gate (47/47, zero detections/timeouts). The
   default request budget caps only current inputs with max side <=640 at 32
   candidates; it preserves caller-stricter limits and leaves dense large-input
   budgets unchanged. Neither slice is representative enough for the
   production FPR gate; BoofCV remains positive-only and cannot be repurposed.
4. **WP-005 decision / WP-013 competitors:** WP-005 is complete: shared-v2
   evidence covers 157 matching images and 727 labels (rebuild 74/727 versus
   main 36/727), and matched macOS Fast Benchmark dispatches passed for both
   refs. Keep the current Model 2 implementation; do not merge rebuild code.
   WP-013 has SHA-pinned local ZBar and offline-built quircs runners. ZBar has
   a 17-image raw monitor observation; quircs has a one-image verified smoke.
   Both still need payload/geometry truth and broader matched manifests before
   a fair competitor comparison.
5. **WP-010:** deterministic raster ordering now removes hash-map variance
   before the contour family's nearby merge. The reproducible `lots` artifact
   reports 104/420 finder and 91/420 grouping recall, but 214 labels still
   have no contained proposal. Preserve the capped primary/ROI/contour
   staging and do not relax thresholds without evidence carried through the
   evaluator.
6. **WP-011/WP-007/WP-014/WP-015:** use their explicit acceptance gaps in
   `TODO.md`; do not upgrade their statuses based on component-only tests.
   `format_extracted` is now a real strict-BCH observation count with distance
   buckets; RS candidate/block attempts, successes, and failures are separate
   from accepted decodes. Collect matched category traces before attempting a
   recovery change or attributing a failure to sampling.
   Fresh traces place bright-spots at the ISO non-zero-remainder-bit gate after
   exact format validation and before RS (48 strict BCH candidates, 7,056
   bounded payload-path rejections). On clean revision `135d311`, glare and
   high-version both reach finder/group/transform but no strict-BCH, remainder,
   or RS evidence; their next work is bounded sampling/geometry observation,
   not decoder recovery. A nine-point gray timing-line translation trial made
   neither target reach BCH/RS and increased their one-image cooperative runs,
   so it was reverted; do not re-enable it without a matched recall gain.
   WP-007 now has a clean V1-M path guard: exactly one strict payload attempt,
   one RS candidate, and no recovery payload attempts. It remains component
   evidence, not a photographic or packet-completion claim. WP-015's current
   audit confirms that minimal hosted features have no normal third-party
   dependencies, but a true `no_std` core needs a shared matrix-core crate and
   target/parity validation; do not claim it is supported.

Run targeted checks while iterating, then record exact commands, metrics, and
remaining work in `TODO.md` before handoff. Full benchmark comparisons belong
on GitHub Actions when the packet explicitly requires them.
