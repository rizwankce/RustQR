use rust_qr::{ImageInput, InputError, PixelFormat, try_detect};

#[test]
fn rejects_zero_dimensions_for_every_pixel_format() {
    for format in [PixelFormat::Grayscale, PixelFormat::Rgb, PixelFormat::Rgba] {
        assert!(matches!(
            try_detect(ImageInput::new(&[], 0, 1, format)),
            Err(InputError::ZeroDimension)
        ));
        assert!(matches!(
            try_detect(ImageInput::new(&[], 1, 0, format)),
            Err(InputError::ZeroDimension)
        ));
    }
}

#[test]
fn rejects_short_tightly_packed_buffers() {
    let cases = [
        (PixelFormat::Grayscale, 5),
        (PixelFormat::Rgb, 17),
        (PixelFormat::Rgba, 23),
    ];

    for (format, actual) in cases {
        let data = vec![255; actual];
        let error = try_detect(ImageInput::new(&data, 3, 2, format)).unwrap_err();
        assert!(matches!(
            error,
            InputError::BufferTooShort {
                required,
                actual: error_actual
            } if required == actual + 1 && error_actual == actual
        ));
    }
}

#[test]
fn rejects_stride_smaller_than_a_pixel_row() {
    let data = [255; 24];
    let error =
        try_detect(ImageInput::new(&data, 3, 2, PixelFormat::Rgba).with_stride(11)).unwrap_err();

    assert!(matches!(
        error,
        InputError::InvalidStride {
            stride: 11,
            minimum: 12
        }
    ));
}

#[test]
fn short_padded_buffer_reports_the_last_required_pixel() {
    // Two RGB rows with a 16-byte stride require 16 bytes to reach row two,
    // then 12 bytes for its pixels. Padding after the final row is optional.
    let data = [255; 27];
    let error =
        try_detect(ImageInput::new(&data, 4, 2, PixelFormat::Rgb).with_stride(16)).unwrap_err();

    assert!(matches!(
        error,
        InputError::BufferTooShort {
            required: 28,
            actual: 27
        }
    ));
}

#[test]
fn accepts_padding_and_trailing_bytes_for_all_formats() {
    let cases = [
        (PixelFormat::Grayscale, 8),
        (PixelFormat::Rgb, 16),
        (PixelFormat::Rgba, 20),
    ];

    for (format, stride) in cases {
        let data = vec![255; stride + 16];
        assert!(
            try_detect(ImageInput::new(&data, 4, 2, format).with_stride(stride)).is_ok(),
            "format {format:?} should accept padded rows and trailing bytes"
        );
    }
}

#[test]
fn rejects_dimension_and_stride_arithmetic_overflow() {
    assert!(matches!(
        try_detect(ImageInput::new(&[], usize::MAX, 2, PixelFormat::Rgba)),
        Err(InputError::DimensionOverflow)
    ));

    assert!(matches!(
        try_detect(
            ImageInput::new(&[], 1, usize::MAX, PixelFormat::Grayscale).with_stride(usize::MAX)
        ),
        Err(InputError::DimensionOverflow)
    ));
}

#[test]
fn legacy_rgb_wrapper_returns_empty_for_invalid_input() {
    assert!(rust_qr::detect(&[], 2, 2).is_empty());
}
