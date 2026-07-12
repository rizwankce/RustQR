# Public input fuzzing

Install `cargo-fuzz` and run the public image entry-point target from the
repository root:

```sh
cargo install cargo-fuzz
cargo fuzz run public_image_input
```

The target keeps valid images small so detector work remains bounded, while
still generating zero dimensions, short and padded buffers, invalid strides,
and arithmetic-overflow dimensions.
