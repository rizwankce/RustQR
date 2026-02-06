# Phase 7.8 Failure-Cluster Triage

Generate ranked miss clusters from a saved reading-rate artifact:

```bash
python3 scripts/triage_failure_clusters.py \
  --artifact <path/to/reading_rate_macos.json> \
  --top-n 8 \
  --out-json docs/failure_cluster_report.json \
  --out-md docs/failure_cluster_report.md
```

Outputs:
- `docs/failure_cluster_report.json` (machine-readable ranked clusters)
- `docs/failure_cluster_report.md` (human action report)

Cluster signatures:
- `no-finders`
- `no-groups`
- `transform-fail`
- `format-fail`
- `rs-fail`
- `payload-fail`
- `over-budget-skip`
- `unknown-fail`
