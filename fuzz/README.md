# Public input fuzzing

Install `cargo-fuzz` and run the public image entry-point target from the
repository root:

```sh
cargo install cargo-fuzz
cargo fuzz run public_image_input
cargo fuzz run matrix_decode
```

The target keeps valid images small so detector work remains bounded, while
still generating zero dimensions, short and padded buffers, invalid strides,
and arithmetic-overflow dimensions.

`matrix_decode` bounds generated matrices to the largest Model 2 symbol
(177×177) and exercises dimension validation, format/version parsing,
deinterleaving, Reed-Solomon correction, payload parsing, and confidence-guided
erasures. `cargo fuzz` uses sanitizer instrumentation on supported hosts. Save
any minimized crash under `fuzz/artifacts/<target>/` and add a deterministic
fixture/test before fixing it.

For a local smoke run, use a short explicit time budget:

```sh
cargo fuzz run public_image_input -- -max_total_time=30
cargo fuzz run matrix_decode -- -max_total_time=30
```
