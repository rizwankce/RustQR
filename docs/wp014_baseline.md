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

## Remaining gates

- Establish at least one successful real multi-code lane through WP-012.
- Use the baseline to identify allocations by stage before changing buffers.
- Add matched one-thread comparisons and stable representative workloads before
  evaluating Rayon, SIMD, or PGO.
- Require the WP-007 and WP-010 correctness/benchmark gates before applying
  WP-014’s latency targets.
