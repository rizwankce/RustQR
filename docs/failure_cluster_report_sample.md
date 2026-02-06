# Failure Cluster Triage Report (Sample)

Synthetic sample output from `scripts/triage_failure_clusters.py`.

| Rank | Signature | Missed Images | Missed QR Weight | Likely Stage | Example |
|------|-----------|---------------|------------------|--------------|---------|
| 1 | rs-fail | 12 | 18 | Reed-Solomon correction / bitstream quality | boofcv/noncompliant/image012.jpg |
| 2 | no-finders | 7 | 7 | binarization/finder detection | boofcv/shadows/image004.jpg |
| 3 | over-budget-skip | 4 | 6 | decode budget manager thresholds | boofcv/lots/image003.jpg |
