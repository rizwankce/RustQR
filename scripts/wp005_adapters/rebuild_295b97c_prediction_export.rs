//! Throwaway WP-005 adapter for scratch_from_scratch_rebuild@295b97c.
//!
//! This is deliberately self-contained: copy it to `src/bin/` in a detached
//! rebuild worktree and build it with `--features tools`.

use image::GenericImageView;
use rust_qr::detect;
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::{env, fs};

const SCHEMA: &str = "rustqr.wp005.prediction-stream.v1";
const PREPROCESSING: &str = "rgb8;triangle-resize;max-dim=1024";
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
    let (mut root, mut limit, mut timeout_ms, mut output, mut commit_sha) =
        (None, None, None, None, None);
    let mut it = env::args().skip(1);
    while let Some(f) = it.next() {
        let v = it.next().unwrap_or_else(|| usage());
        match f.as_str() {
            "--root" => root = Some(PathBuf::from(v)),
            "--limit" => limit = v.parse().ok(),
            "--timeout-ms" => timeout_ms = v.parse().ok(),
            "--output" => output = Some(PathBuf::from(v)),
            "--commit-sha" => commit_sha = Some(v),
            _ => usage(),
        }
    }
    let a = Args {
        root: root.unwrap_or_else(|| usage()),
        limit: limit.unwrap_or_else(|| usage()),
        timeout_ms: timeout_ms.unwrap_or_else(|| usage()),
        output: output.unwrap_or_else(|| usage()),
        commit_sha: commit_sha.unwrap_or_else(|| usage()),
    };
    if a.timeout_ms != 0 || a.limit == 0 {
        usage()
    }
    a
}
fn json(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if c.is_control() => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
fn images(root: &Path) -> Vec<PathBuf> {
    let mut st = vec![root.to_path_buf()];
    let mut out = Vec::new();
    while let Some(d) = st.pop() {
        let Ok(es) = fs::read_dir(d) else { continue };
        for e in es.flatten() {
            let p = e.path();
            if p.is_dir() {
                st.push(p)
            } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| {
                matches!(
                    x.to_ascii_lowercase().as_str(),
                    "jpg" | "jpeg" | "png" | "gif" | "bmp"
                )
            }) {
                out.push(p)
            }
        }
    }
    out.sort();
    out
}
fn fingerprint(root: &Path) -> String {
    let mut h = 0xcbf29ce484222325u64;
    for p in images(root) {
        let r = p.strip_prefix(root).unwrap().to_string_lossy();
        for bs in [r.as_bytes(), &[0u8][..], &fs::read(&p).unwrap_or_default()].into_iter() {
            for b in bs {
                h ^= u64::from(*b);
                h = h.wrapping_mul(0x100000001b3)
            }
        }
    }
    format!("fnv1a64:{h:016x}")
}
fn main() {
    let a = parse_args();
    let fp = fingerprint(&a.root);
    let mut rows = Vec::new();
    let mut n = std::collections::BTreeMap::<String, usize>::new();
    for p in images(&a.root) {
        let id = p
            .strip_prefix(&a.root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let category = id.split('/').next().unwrap_or("").to_owned();
        if n.get(&category).copied().unwrap_or(0) >= a.limit {
            continue;
        }
        let total = Instant::now();
        let original = match image::open(&p) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("skip {}: {e}", p.display());
                continue;
            }
        };
        let (ow, oh) = original.dimensions();
        let work = if ow.max(oh) > 1024 {
            original.resize_exact(
                ((ow as f64 * 1024.0 / ow.max(oh) as f64).round().max(1.0)) as u32,
                ((oh as f64 * 1024.0 / ow.max(oh) as f64).round().max(1.0)) as u32,
                image::imageops::FilterType::Triangle,
            )
        } else {
            original
        };
        let rgb = work.to_rgb8();
        let (ww, wh) = rgb.dimensions();
        let begin = Instant::now();
        let found = detect(rgb.as_raw(), ww as usize, wh as usize);
        let core = begin.elapsed().as_secs_f64() * 1000.0;
        let (sx, sy) = (ow as f64 / ww as f64, oh as f64 / wh as f64);
        let qs = found
            .iter()
            .map(|q| {
                format!(
                    "[[{:.6},{:.6}],[{:.6},{:.6}],[{:.6},{:.6}],[{:.6},{:.6}]]",
                    q.corners[0].x as f64 * sx,
                    q.corners[0].y as f64 * sy,
                    q.corners[1].x as f64 * sx,
                    q.corners[1].y as f64 * sy,
                    q.corners[2].x as f64 * sx,
                    q.corners[2].y as f64 * sy,
                    q.corners[3].x as f64 * sx,
                    q.corners[3].y as f64 * sy
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let ps = found
            .iter()
            .map(|q| json(&q.payload))
            .collect::<Vec<_>>()
            .join(",");
        rows.push(format!("{{\"image_id\":{},\"category\":{},\"original_width\":{},\"original_height\":{},\"working_width\":{},\"working_height\":{},\"predicted_quadrilaterals\":[{}],\"payloads\":[{}],\"core_elapsed_ms\":{:.6},\"end_to_end_elapsed_ms\":{:.6},\"timed_out\":false}}",json(&id),json(&category),ow,oh,ww,wh,qs,ps,core,total.elapsed().as_secs_f64()*1000.0));
        *n.entry(category).or_default() += 1;
    }
    let out = format!(
        "{{\"schema_version\":{},\"metadata\":{{\"commit_sha\":{},\"dataset_fingerprint\":{},\"preprocessing_fingerprint\":{},\"limit_per_category\":{}}},\"images\":[{}]}}",
        json(SCHEMA),
        json(&a.commit_sha),
        json(&fp),
        json(PREPROCESSING),
        a.limit,
        rows.join(",")
    );
    if let Some(parent) = a.output.parent() {
        fs::create_dir_all(parent).expect("create output directory")
    }
    fs::write(a.output, out).expect("write output")
}
