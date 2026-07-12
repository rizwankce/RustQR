#![no_main]

use libfuzzer_sys::fuzz_target;
use rust_qr::{ImageInput, PixelFormat, try_detect};

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }

    let format = match data[0] % 3 {
        0 => PixelFormat::Grayscale,
        1 => PixelFormat::Rgb,
        _ => PixelFormat::Rgba,
    };
    let width = dimension(data[1], data[0]);
    let height = dimension(data[2], data[3]);
    let payload = &data[4..];
    let input = ImageInput::new(payload, width, height, format);

    if data[3] & 1 == 0 {
        let stride = if data[3] & 2 == 0 {
            (data[3] as usize).saturating_mul(data[1] as usize)
        } else {
            usize::MAX
        };
        let _ = try_detect(input.with_stride(stride));
    } else {
        let _ = try_detect(input);
    }
});

fn dimension(value: u8, selector: u8) -> usize {
    match selector & 0x0f {
        0 => 0,
        1 => usize::MAX,
        _ => usize::from(value % 33),
    }
}
