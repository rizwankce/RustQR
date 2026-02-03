use rust_qr::tools::{bench_limit_from_env, dataset_iter, dataset_root_from_env, smoke_from_env};
use std::path::PathBuf;

pub fn collect_dataset_images() -> (PathBuf, Vec<PathBuf>) {
    let root = dataset_root_from_env();
    let limit = bench_limit_from_env();
    let smoke = smoke_from_env();

    let images: Vec<PathBuf> = dataset_iter(&root, limit, smoke).collect();
    (root, images)
}
