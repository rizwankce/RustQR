use rust_qr::detect_with_report;

#[path = "../tools/mod.rs"]
mod tools;

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("smoke") => {
            let image = vec![0u8; 3 * 4 * 4];
            let report = detect_with_report(&image, 4, 4);
            println!(
                "codes={} failure={:?}",
                report.codes.len(),
                report.failure_signature
            );
        }
        Some("reading-rate") => {
            let subcommand_args = args.collect::<Vec<_>>();
            match tools::parse_reading_rate_args(&subcommand_args) {
                Ok(tools::ReadingRateCommand::Help) => {
                    println!("{}", tools::reading_rate_usage());
                }
                Ok(tools::ReadingRateCommand::Run(parsed)) => {
                    let report = match tools::build_reading_rate_report(&parsed) {
                        Ok(report) => report,
                        Err(err) => {
                            eprintln!("reading-rate error: {err}");
                            std::process::exit(1);
                        }
                    };

                    println!("{}", tools::render_console_summary(&report));

                    let json = tools::report_to_json(&report);
                    if let Err(err) = tools::write_report_json(&parsed.artifact_path, &json) {
                        eprintln!(
                            "reading-rate error: failed to write artifact {}: {}",
                            parsed.artifact_path.display(),
                            err
                        );
                        std::process::exit(1);
                    }

                    println!("artifact={}", parsed.artifact_path.display());
                }
                Err(err) => {
                    eprintln!("reading-rate error: {err}");
                    std::process::exit(2);
                }
            }
        }
        _ => {
            eprintln!("RustQR scaffold CLI");
            eprintln!("usage: qrtool smoke");
            eprintln!(
                "       qrtool reading-rate [--profile monitor-smoke|nominal-smoke] [--dataset-root PATH] [--artifact PATH] [--limit N] [--max-working-dim N] [--emergency-cutoff-ms N]"
            );
            std::process::exit(2);
        }
    }
}
