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
   It already reaches 34/60 finder and 29/60 grouped symbols before legacy
   fast/brightness early returns; global bypass trials were only 1/60 at 1.16
   s or 4/60 at 1.02 s and were reverted. Do not disable them globally.
   A one-frontier adaptive remainder also stayed 1/60 at 922.5 ms; do not
   repeat it.
   Candidate-count-only promotion to `MultiQrHeavy` was also 1/60 despite
   90–91 ranked candidates, 13 regions, and 37 attempts; routing alone is not
   the missing step.
   The route audit's
   five Otsu-only payloads (`011`, `014`, `039`, `043`, `046`) and two absent
   from both routes (`021`, `047`) remain useful diagnosis, but global or
   residual Otsu retries are not viable under the bounded request policy.
   A residual-Otsu proposal with eight reserved decode attempts regressed to
   40/50 in 911.42 ms and was reverted; do not repeat that design. A later
   geometry anti-join also held 48/50: `021` and `047` have no Otsu proposal,
   so further Otsu-only selection cannot close the controlled gap.
   A dense-gated white-ring component source improved realistic `lots/image001`
   proposal/grouping recall from 34/60 and 29/60 to 44/60 and 31/60 with no
   added spurious proposals, but the public brightness route still returns
   through its legacy path. Telemetry now proves that path reaches transform,
   BCH, and RS but has 1,320 timing-gate rejections (mean H/V about .23/.25).
   A shared-deadline bridge was slower and worse (0/60, 690ms); a central
   finder-template threshold plane had 45 eligible samples but zero gate
   passes. Both were reverted. Do not bypass the brightness return, relax the
   timing gate, or repeat the threshold-plane probe without new evidence.
   The new opt-in candidate-stage trace makes the residual explicit on
   `lots/image001`: 46 attempted groups, 45 timing-gate-only failures, and one
   accepted BCH/RS path (1,320 timing rejections total). Continue with a
   base-versus-selected transform observation, not a new proposal route.
2. **WP-011 geometry/sampling:** expiry now prevents scheduling new image-wide
   passes, but scans already in flight cannot be interrupted. Continue only
   with bounded category-specific geometry or cancellable-stage evidence.
   The next safe diagnostic compares base versus selected refinement transforms
   on one high-version/glare proposal; high-version's near-gate timing ratios
   make it the most informative first case.
   The trace-only base/selected transform observation is now present. On
   high-version, accepted refinement can raise geometry quality without
   raising timing ratios above the 0.60 gate; do not treat refinement as a
   sampling fix or loosen that gate.
3. **WP-009 safety:** synthetic baseline is present; acceptance requires a
   licensed, representative negative corpus and explicit false-positive
   budget. A 47-image/12.5184 MP Apache-2.0 ZXing high-contrast negative
   slice is now vendored with per-asset hashes, provenance, and zero-timeout
   release public-API evidence; it is intentionally ignored in debug because
   its five-second request budget is release-qualified. A separate 17 Aztec/23 Data Matrix/7 Code128 ZXing
   slice has complete membership/format/hash provenance and now passes its
   strict five-second release gate (47/47, zero detections/timeouts). The
   default request budget caps only current inputs with max side <=640 at 32
   candidates; it preserves caller-stricter limits and leaves dense large-input
   budgets unchanged. Neither slice is representative enough for the
   production FPR gate; BoofCV remains positive-only and cannot be repurposed.
   A one-image CC-BY-SA-4.0 Wikimedia Commons photographed-screen slice is now
   vendored with a permanent oldid, author, source SHA-1, local SHA-256,
   dimensions, attribution, and manual zero-QR annotation. Its strict release
   gate passes (1/1, 16.036032 MP, zero detections/timeouts in 2.88 s); debug
   exceeds the five-second cooperative deadline, so it is release-only and
   ignored by default. A separate CC-BY-3.0 Commons book-shelf photo now adds a
   300x450 photographed-text slice with the same per-asset provenance and
   release-only qualification (zero detections/timeouts in 0.35 s). A third
   CC-BY-2.0 packaging photo is release-qualified (zero detections/timeouts in
   1.23 s). The initial category coverage is now screen/text/packaging, but an
   agreed production FPR budget and representative scale remain open.
4. **WP-005 decision / WP-013 competitors:** WP-005 is complete: shared-v2
   evidence covers 157 matching images and 727 labels (rebuild 74/727 versus
   main 36/727), and matched macOS Fast Benchmark dispatches passed for both
   refs. Keep the current Model 2 implementation; do not merge rebuild code.
   WP-013 has SHA-pinned local ZBar and offline-built quircs runners. A six
   fixture shared-PGM subset supplies exact payload truth for RustQR and both
   runners, and geometry-v1 records native corners against a generator-derived
   reference. All 18 payloads match and quircs reaches 1.0 IoU, but RustQR/ZBar
   miss the strict 0.99 gate on named fixtures; do not soften it or claim
   geometry parity. The shared-PGM protocol remains neither real-scene
   localization evidence nor an end-to-end timing boundary.
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
   evidence, not a photographic or packet-completion claim. WP-015 now has a
   separately built `rustqr-matrix-core` `no_std + alloc` crate for strict and
   known-erasure matrix decoding, with core/host generated-corpus and mutation
   parity. Bare-metal compilation is CI-gated because the local target is not
   installed; manual CI run 29488989514 passed that check together with MSRV,
   format, clippy, and Linux/macOS/Windows library tests. Hosted confidence/fallback/beam recovery, deadline/cancellation,
   and `Instant` remain outside core; do not claim full decoder `no_std` or
   move them without a dedicated policy/callback API.
   WP-014 also removed the rank-frontier copy without changing group order or
   caps; preserve it as allocation-only evidence, not a latency claim.

Run targeted checks while iterating, then record exact commands, metrics, and
remaining work in `TODO.md` before handoff. Full benchmark comparisons belong
on GitHub Actions when the packet explicitly requires them.
