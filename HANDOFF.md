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
   budget. A 47-image/12.5184 MP Apache-2.0 ZXing high-contrast negative
   slice is now vendored with per-asset hashes, provenance, and zero-timeout
   public-API evidence. It is not representative enough for the production
   FPR gate; BoofCV remains positive-only and cannot be repurposed.
4. **WP-005 decision / WP-013 competitors:** WP-005 is complete: shared-v2
   evidence covers 157 matching images and 727 labels (rebuild 74/727 versus
   main 36/727), and matched macOS Fast Benchmark dispatches passed for both
   refs. Keep the current Model 2 implementation; do not merge rebuild code.
   WP-013 has a SHA-pinned local ZBar runner and a 17-image raw monitor
   observation, but needs additional pinned runners plus payload/geometry
   truth before a fair competitor comparison.
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
   Fresh traces place bright-spots at the ISO non-zero-remainder-bit gate after
   exact format validation and before RS (48 strict BCH candidates, 7,056
   bounded payload-path rejections), while glare/high-version do not reach
   decoder evidence at all.
   WP-007 now has a clean V1-M path guard: exactly one strict payload attempt,
   one RS candidate, and no recovery payload attempts. It remains component
   evidence, not a photographic or packet-completion claim. WP-015's current
   audit confirms that minimal hosted features have no normal third-party
   dependencies, but a true `no_std` core needs a shared matrix-core crate and
   target/parity validation; do not claim it is supported.

Run targeted checks while iterating, then record exact commands, metrics, and
remaining work in `TODO.md` before handoff. Full benchmark comparisons belong
on GitHub Actions when the packet explicitly requires them.
