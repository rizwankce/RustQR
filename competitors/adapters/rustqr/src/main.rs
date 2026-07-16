use std::{env, fs};

use rust_qr::{DecoderOptions, DecoderPreset, ImageInput, PixelFormat, try_detect_with_options};

fn token(bytes: &[u8], index: &mut usize) -> Option<Vec<u8>> {
    loop {
        while *index < bytes.len() && bytes[*index].is_ascii_whitespace() {
            *index += 1;
        }
        if *index >= bytes.len() || bytes[*index] != b'#' {
            break;
        }
        while *index < bytes.len() && bytes[*index] != b'\n' {
            *index += 1;
        }
    }
    let start = *index;
    while *index < bytes.len() && !bytes[*index].is_ascii_whitespace() {
        *index += 1;
    }
    (start != *index).then(|| bytes[start..*index].to_vec())
}

fn pgm(path: &str) -> Result<(usize, usize, Vec<u8>), Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let mut index = 0;
    if token(&bytes, &mut index).as_deref() != Some(b"P5") {
        return Err("expected binary PGM (P5)".into());
    }
    let width: usize =
        std::str::from_utf8(&token(&bytes, &mut index).ok_or("missing width")?)?.parse()?;
    let height: usize =
        std::str::from_utf8(&token(&bytes, &mut index).ok_or("missing height")?)?.parse()?;
    if token(&bytes, &mut index).as_deref() != Some(b"255") {
        return Err("expected 8-bit PGM".into());
    }
    if index >= bytes.len() || !bytes[index].is_ascii_whitespace() {
        return Err("missing PGM raster separator".into());
    }
    index += 1;
    let pixels = bytes[index..].to_vec();
    if pixels.len() != width.checked_mul(height).ok_or("PGM dimensions overflow")? {
        return Err("PGM raster length does not match dimensions".into());
    }
    Ok((width, height, pixels))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args().skip(1);
    let geometry = matches!(arguments.next().as_deref(), Some("--geometry"));
    let argument = if geometry {
        arguments.next()
    } else {
        // Keep the original payload-lines protocol as the default so the
        // existing monitor and payload conformance runners remain stable.
        env::args().nth(1)
    }
    .ok_or("expected PGM path or --version")?;
    if argument == "--version" {
        println!("rust_qr {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let (width, height, pixels) = pgm(&argument)?;
    let result = try_detect_with_options(
        ImageInput::new(&pixels, width, height, PixelFormat::Grayscale),
        DecoderOptions::with_preset(DecoderPreset::Balanced),
    )?;
    if result.codes.is_empty() {
        std::process::exit(1);
    }
    for code in result.codes {
        if geometry {
            // Delimiter-safe, ASCII-only protocol for the synthetic shared-PGM
            // geometry runner: `G\t<payload hex>\t<x,y;...>`.  Payload bytes
            // are hexadecimal so neither tabs nor newlines are ambiguous.
            let corners = code
                .position
                .map(|point| format!("{:.6},{:.6}", point.x, point.y));
            println!("G\t{}\t{}", hex(&code.data), corners.join(";"));
        } else {
            println!("{}", String::from_utf8_lossy(&code.data));
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        text.push(DIGITS[(byte >> 4) as usize] as char);
        text.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    text
}
