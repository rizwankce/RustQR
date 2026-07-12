# GitHub run 21927167412 benchmark evidence

These files are a preserved, incomplete copy of historical evidence from
[GitHub Actions run 21927167412](https://github.com/rizwankce/RustQR/actions/runs/21927167412).
They are **not an authoritative baseline for `main`** and must not be used as a
regression-gate input without an explicit compatibility review.

## Provenance

- Source branch: `scratch_from_scratch_rebuild`
- Source commit: `e735ac991fb27e4c8b3c5c70d7e51c7fa3f61de6`
  (`bench: add full boofcv profiles and all-profiles workflow mode`)
- Trigger: manual `workflow_dispatch`, profile `all-profiles`, limit `0`
- Run interval: 2026-02-11 23:27:46 UTC to 2026-02-12 00:11:09 UTC
- Runner: GitHub-hosted `ubuntu-24.04`, image `20260201.15.1`, x86_64
- Toolchain: `rustc 1.93.0 (254b59607 2026-01-19)`
- Dependency lock fingerprint: SHA-256
  `186981af2e3311c64577f7e2099527457838e9d65c523dea13f3fc287c662794`
  for `Cargo.lock` at the source commit

The workflow ran each profile in a debug Cargo build using this command shape:

```text
cargo run --locked --bin qrtool -- reading-rate \
  --artifact benchmark/all_profiles/reading_rate_<profile>.json \
  --profile <profile> --max-working-dim 1024 \
  --emergency-cutoff-ms 1400
```

## Independent fingerprints

The fingerprints intentionally identify different inputs. Do not substitute
one for another.

| Kind | SHA-256 | Definition |
| --- | --- | --- |
| Dataset images | `1798bfa27fb55f0c42971f55d7863290a1a7f9dee507a2a00114dced516d654a` | SHA-256 of the sorted lines `<git-blob-id><two spaces><path>` for the 562 image files under `benches/images` at the source commit |
| Evaluator | `096e5183bd6f1a359f387556e9274403c22a575753421f4dbdcd5ee18d02577a` | SHA-256 of `src/tools/mod.rs` at the source commit; artifact schema is `wp007-reading-rate-v1` |
| Preprocessing/pipeline | `6e442f12fe7aca00c7a2ae2180335190737e78d3dc0bfa2a1b91728746957e36` | SHA-256 of `src/pipeline/mod.rs` at the source commit, used with max working dimension 1024 and 1400 ms emergency cutoff |
| Preserved artifact set | `a75d969194ec234787341934551e83f590e099a3412e9546d9d5642f13705784` | SHA-256 of sorted lines from `SHA256SUMS` for the 17 preserved JSON files |

To reproduce the dataset image fingerprint from the source commit:

```bash
git ls-tree -r e735ac991fb27e4c8b3c5c70d7e51c7fa3f61de6 benches/images \
  | awk 'tolower($4) ~ /\.(png|jpg|jpeg|gif|bmp|tif|tiff|webp)$/ {print $3 "  " $4}' \
  | LC_ALL=C sort | shasum -a 256
```

Only image paths and Git blob contents participate. Label files, backup label
files, Markdown documentation, and other unrelated files therefore cannot
change this dataset fingerprint. Labels remain evaluator inputs and should be
fingerprinted separately by a future evaluator schema.

## Evaluator and timing boundaries

- `boofcv-*` profiles use BoofCV point-annotation labels. A case matched when
  at least one QR code was returned. This is an annotation/count-based
  detection lane; it did not validate payloads or match quadrilaterals to
  annotations one-to-one.
- `payload-validated` uses custom payload labels. A case matched only when a
  returned payload equaled the normalized expected payload.
- `runtime_ms` starts before image load and optional resize, then includes the
  detection pipeline and payload/count evaluation. It stops before report
  aggregation and JSON serialization. The pipeline may stop work at the
  configured emergency cutoff, but elapsed case time can exceed 1400 ms.
- Images whose largest dimension exceeded 1024 were resized exactly with the
  image crate's Triangle filter before detection.

These limitations are why the artifacts describe historical rebuild-branch
evidence, not a trustworthy comparison baseline for the current evaluator.

## Preservation decision

Keep this small evidence set in the repository with this provenance record and
checksums. The original Actions artifact used the default 90-day retention and
has expired; it cannot now serve as the sole copy. Future large/full artifacts
should remain Actions or release assets with retention chosen explicitly, while
the repository keeps a provenance manifest and any milestone summary needed for
review. Never overwrite this directory: create a new run-ID directory.

The workflow log shows that `reading_rate_boofcv-all.json` was generated and
uploaded, but it is absent from this preserved copy. The other 17 JSON results
are retained unchanged. This gap is documented rather than silently replacing
or reconstructing the missing result.
