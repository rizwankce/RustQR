# WP-014 performance baseline protocol

WP-014 does not make an optimization or performance-target claim yet. This
document defines the reproducible baseline needed before profiling work can be
ranked safely.

## Lanes and boundaries

The probe uses one checked-in BoofCV image per lane:

| Lane | Image | Current success requirement | Purpose |
| --- | --- | --- | --- |
| clean | `nominal/image005.jpg` | Must decode | Common successful image path |
| hard | `damaged/image002.jpg` | Must decode | Successful recovery path |
| multi_candidate | `lots/image001.jpg` | Does not currently decode | Dense-scene control only |

The `multi_candidate` lane is not a successful multi-code latency result. On
the current revision, all seven `lots` images returned zero symbols in a direct
`qrtool detect` check. WP-012 must provide a successful dense-scene fixture
before WP-014 can evaluate or claim multi-code latency.

The collection tool records two distinct boundaries:

- **In-process detect-only:** after RGB bytes are loaded, through
  `rust_qr::detect` returning. It records first-call time, steady repeated-call
  time, allocation/reallocation counts and requested bytes, and throughput.
- **Fresh-process end-to-end:** `qrtool` startup, image loading, detection, and
  output formatting. It records process-isolated wall time plus a best-effort
  sampled RSS peak.

Neither boundary is interchangeable with reading-rate artifacts, which score a
corpus and use separate timing semantics.

## Run

```bash
python3 scripts/collect_wp014_baseline.py \
  --process-runs 5 \
  --in-process-iterations 5 \
  --output benchmark/wp014/local-baseline.json
```

The JSON output includes the commit, dirty-worktree state, host metadata,
commands, case paths, raw observations, and median/p95 process data. Do not
compare artifacts across different hosts, datasets, or measurement boundaries.

For a fast probe while iterating:

```bash
python3 scripts/collect_wp014_baseline.py \
  --process-runs 1 \
  --in-process-iterations 1 \
  --output /tmp/wp014-smoke.json
```

## Initial local evidence

On 2026-07-12, the allocation-only probe with one warm iteration completed:

| Lane | First-call result | First-call allocations | First-call requested bytes |
| --- | ---: | ---: | ---: |
| clean | 1 symbol | 13,711 | 2,158,004 |
| hard | 1 symbol | 758,042 | 90,630,950 |
| multi_candidate | 0 symbols | 17,561,330 | 1,182,535,236 |

These values are machine-, revision-, and input-specific. They identify the
large hard/dense allocation surfaces for later profiling; they are not a
performance comparison or an acceptance result.

## Controlled dense-lane allocation probe

WP-012's checked-in `density_050.png` is a successful 50-symbol lane, so the
probe also supports measuring it in isolation:

```bash
WP014_ALLOCATION_ONLY=1 WP014_PROFILE_ITERATIONS=1 \
WP014_PROFILE_LANE=dense_50 \
cargo bench --bench wp014_profiles --features tools -- --noplot
```

On the local 2026-07-13 release probe, the unoptimized path decoded 48
symbols in 1.742 s and made 451,223 allocations (246,725 reallocations,
95,470,388 requested bytes). Replacing the two fixed per-candidate temporary
vectors—25 bottom-right hypotheses and at-most-five version hypotheses—with
stack arrays retained the same 48 decoded symbols and reduced that to 451,087
allocations (246,521 reallocations, 95,437,204 requested bytes). Its single
warm call was 1.726 s. The allocation reduction is deterministic for this
input; the 0.9% latency difference is only a single-run observation, not a
throughput claim.

### Dense recovery gate follow-up

The next allocation probe isolated a separate per-region cost: the image
candidate loop passed `allow_heavy_recovery = false` after its first two
bounded attempts, but its sampled-matrix decoder still entered non-canonical
format/traversal and confidence-beam recovery after every strict failure.
Those fallbacks are useful for the two explicitly selected recovery attempts;
they are unnecessary work for the remaining clean dense candidates.

The sampled-matrix entry point now always runs the strict ISO path, while
gating only those fallback hypotheses behind the existing
`allow_heavy_recovery` budget. The three-iteration release probe on the same
fixture retained 48 decodes per call and reduced per-call allocations from
451,087 to 444,857 (reallocations 246,521 to 246,192; requested bytes
95,437,204 to 94,802,508). Warm time was 1.725 s before and 1.732 s after,
which is within this small local run's noise; this is an allocation and bounded
work reduction, not a latency claim. The dense output, candidate cap, and
strict-path behavior are unchanged.

## 1024-pixel credibility sample (2026-07-13)

`artifacts/wp014_credibility_1024_a60d082.json` is a bounded five-process-run,
five-warm-call sample collected at commit `a60d082` with `QR_MAX_DIM=1024`:

```bash
QR_MAX_DIM=1024 python3 -B scripts/collect_wp014_baseline.py \
  --skip-build --process-runs 5 --in-process-iterations 5 \
  --lanes clean,hard,dense_50 \
  --output artifacts/wp014_credibility_1024_a60d082.json
```

| Lane | Process p50 | Process p95 | Warm detect-only average | Result | Credibility-target reading |
| --- | ---: | ---: | ---: | --- | --- |
| clean | 54.50 ms | 266.26 ms | 37.46 ms | 1/1 in all five runs | Does not pass the 100 ms p95 bound. |
| hard | 2516.36 ms | 2717.33 ms | unavailable | 0/1 in all five runs | Not a successful lane; the profile's required-success assertion aborts. |
| controlled `dense_50` | 354.12 ms | 355.46 ms | 369.69 ms | 48/50 direct results in all five runs | Does not pass the 25 ms p50 or 100 ms p95 bounds. |

The controlled dense source is 1276x1119 and was Triangle-resized to
1024x898 before detection, so its process values use the stated 1024-pixel
boundary. Clean and hard are already below that limit (550x309 and 1008x756).
The artifact preserves each raw process duration and decoded count.

The allocation probe deliberately emits only one aggregate duration for its
five warm calls; it cannot establish in-process p50/p95. The process-isolated
distribution above is the available p50/p95 evidence. Five samples are a
bounded credibility check, not a stable throughput or regression claim.

The collector now accepts `--lanes` and runs the allocation probe one lane at
a time. That both makes the successful controlled dense route explicit and
preserves a required lane's failed profile as an artifact error instead of
misreporting it as a successful timing result.

## Remaining gates

- Establish at least one successful real multi-code lane through WP-012.
- Use the baseline to identify allocations by stage before changing buffers.
- Add matched one-thread comparisons and stable representative workloads before
  evaluating Rayon, SIMD, or PGO.
- Require the WP-007 and WP-010 correctness/benchmark gates before applying
  WP-014’s latency targets.
