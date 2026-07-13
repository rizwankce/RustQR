//! Throwaway WP-005 adapter for main@5b9b41e.
//!
//! Copy this file to `src/bin/wp005_prediction_export.rs` in a detached
//! worktree.  It intentionally has no dependency on the active branch.

use image::GenericImageView;
use rust_qr::detect;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

const SCHEMA: &str = "rustqr.wp005.prediction-stream.v1";
const PREPROCESSING: &str = "rgb8;triangle-resize;max-dim=1024";

#[derive(Debug)]
struct Args {
    root: PathBuf,
    limit: usize,
    timeout_ms: u64,
    output: PathBuf,
    commit_sha: String,
}

fn usage() -> ! {
    eprintln!(
        "usage: wp005_prediction_export --root PATH --limit N --timeout-ms 0 --commit-sha SHA --output PATH"
    );
    std::process::exit(2)
}

fn parse_args() -> Args {
    let mut root = None;
    let mut limit = None;
    let mut timeout_ms = None;
    let mut output = None;
    let mut commit_sha = None;
    let mut it = env::args().skip(1);
    while let Some(flag) = it.next() {
        let value = it.next().unwrap_or_else(|| usage());
        match flag.as_str() {
            "--root" => root = Some(PathBuf::from(value)),
            "--limit" => limit = value.parse().ok(),
            "--timeout-ms" => timeout_ms = value.parse().ok(),
            "--output" => output = Some(PathBuf::from(value)),
            "--commit-sha" => commit_sha = Some(value),
            _ => usage(),
        }
    }
    let args = Args {
        root: root.unwrap_or_else(|| usage()),
        limit: limit.unwrap_or_else(|| usage()),
        timeout_ms: timeout_ms.unwrap_or_else(|| usage()),
        output: output.unwrap_or_else(|| usage()),
        commit_sha: commit_sha.unwrap_or_else(|| usage()),
    };
    if args.timeout_ms != 0 || args.limit == 0 {
        usage()
    }
    args
}

fn json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn images(root: &Path) -> Vec<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    let mut result = Vec::new();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|s| s.to_str()).is_some_and(|x| {
                matches!(
                    x.to_ascii_lowercase().as_str(),
                    "jpg" | "jpeg" | "png" | "gif" | "bmp"
                )
            }) {
                result.push(path);
            }
        }
    }
    result.sort();
    result
}

fn fingerprint(root: &Path) -> String {
    let mut h = 0xcbf29ce484222325u64;
    for path in images(root) {
        let rel = path.strip_prefix(root).unwrap().to_string_lossy();
        for bytes in [
            rel.as_bytes(),
            &[0u8][..],
            &fs::read(&path).unwrap_or_default(),
        ]
        .into_iter()
        {
            for b in bytes {
                h ^= u64::from(*b);
                h = h.wrapping_mul(0x100000001b3);
            }
        }
    }
    format!("fnv1a64:{h:016x}")
}

fn main() {
    let args = parse_args();
    let dataset_fingerprint = fingerprint(&args.root);
    let mut rows = Vec::new();
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for path in images(&args.root) {
        let rel = path
            .strip_prefix(&args.root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let category = rel.split('/').next().unwrap_or("").to_owned();
        if counts.get(&category).copied().unwrap_or(0) >= args.limit {
            continue;
        }
        let start = Instant::now();
        let original = match image::open(&path) {
            Ok(value) => value,
            Err(err) => {
                eprintln!("skip {}: {err}", path.display());
                continue;
            }
        };
        let (ow, oh) = original.dimensions();
        let working = if ow.max(oh) > 1024 {
            original.resize(1024, 1024, image::imageops::FilterType::Triangle)
        } else {
            original
        };
        let rgb = working.to_rgb8();
        let (ww, wh) = rgb.dimensions();
        let before_core = Instant::now();
        let found = detect(rgb.as_raw(), ww as usize, wh as usize);
        let core = before_core.elapsed().as_secs_f64() * 1000.0;
        let sx = ow as f64 / ww as f64;
        let sy = oh as f64 / wh as f64;
        // The historical fast path can return a decoded payload with an all-zero
        // `position`. It is not a localizable prediction, so exclude it as a
        // pair rather than emitting invalid geometry or a payload-only row.
        let found = found
            .into_iter()
            .filter(|code| {
                let p = &code.position;
                let area = (p[0].x * p[1].y - p[1].x * p[0].y)
                    + (p[1].x * p[2].y - p[2].x * p[1].y)
                    + (p[2].x * p[3].y - p[3].x * p[2].y)
                    + (p[3].x * p[0].y - p[0].x * p[3].y);
                area.abs() > 1e-3
            })
            .collect::<Vec<_>>();
        let quads = found
            .iter()
            .map(|code| {
                format!(
                    "[[{:.6},{:.6}],[{:.6},{:.6}],[{:.6},{:.6}],[{:.6},{:.6}]]",
                    code.position[0].x as f64 * sx,
                    code.position[0].y as f64 * sy,
                    code.position[1].x as f64 * sx,
                    code.position[1].y as f64 * sy,
                    code.position[2].x as f64 * sx,
                    code.position[2].y as f64 * sy,
                    code.position[3].x as f64 * sx,
                    code.position[3].y as f64 * sy
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let payloads = found
            .iter()
            .map(|code| json(&code.content))
            .collect::<Vec<_>>()
            .join(",");
        rows.push(format!("{{\"image_id\":{},\"category\":{},\"original_width\":{},\"original_height\":{},\"working_width\":{},\"working_height\":{},\"predicted_quadrilaterals\":[{}],\"payloads\":[{}],\"core_elapsed_ms\":{:.6},\"end_to_end_elapsed_ms\":{:.6},\"timed_out\":false}}", json(&rel), json(&category), ow, oh, ww, wh, quads, payloads, core, start.elapsed().as_secs_f64()*1000.0));
        *counts.entry(category).or_default() += 1;
    }
    let output = format!(
        "{{\"schema_version\":{},\"metadata\":{{\"commit_sha\":{},\"dataset_fingerprint\":{},\"preprocessing_fingerprint\":{},\"limit_per_category\":{}}},\"images\":[{}]}}",
        json(SCHEMA),
        json(&args.commit_sha),
        json(&dataset_fingerprint),
        json(PREPROCESSING),
        args.limit,
        rows.join(",")
    );
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent).expect("create output directory");
    }
    fs::write(&args.output, output).expect("write output");
}
