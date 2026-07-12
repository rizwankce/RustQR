use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).ok_or("expected a PGM path")?;
    let image = image::open(path)?.to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(image);
    let grids = prepared.detect_grids();
    let mut decoded = 0usize;
    for grid in grids {
        if let Ok((_meta, content)) = grid.decode() {
            println!("{}", content);
            decoded += 1;
        }
    }
    if decoded == 0 { std::process::exit(1); }
    Ok(())
}
