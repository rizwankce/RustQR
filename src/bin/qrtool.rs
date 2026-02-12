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
                    if report.kpi_gate.pass == Some(false) {
                        eprintln!("reading-rate KPI gate failed:");
                        for failure in &report.kpi_gate.failures {
                            eprintln!("kpi-gate-failure: {failure}");
                        }
                        std::process::exit(3);
                    }
                }
                Err(err) => {
                    eprintln!("reading-rate error: {err}");
                    std::process::exit(2);
                }
            }
        }
        Some("benchdiff") => {
            let subcommand_args = args.collect::<Vec<_>>();
            match tools::parse_benchdiff_args(&subcommand_args) {
                Ok(tools::BenchdiffCommand::Help) => {
                    println!("{}", tools::benchdiff_usage());
                }
                Ok(tools::BenchdiffCommand::Run(parsed)) => {
                    let report = match tools::build_benchdiff_report(&parsed) {
                        Ok(report) => report,
                        Err(err) => {
                            eprintln!("benchdiff error: {err}");
                            std::process::exit(1);
                        }
                    };

                    println!("{}", tools::render_benchdiff_console_summary(&report));

                    let json = tools::benchdiff_to_json(&report);
                    if let Err(err) = tools::write_report_json(&parsed.artifact_path, &json) {
                        eprintln!(
                            "benchdiff error: failed to write artifact {}: {}",
                            parsed.artifact_path.display(),
                            err
                        );
                        std::process::exit(1);
                    }

                    println!("artifact={}", parsed.artifact_path.display());
                }
                Err(err) => {
                    eprintln!("benchdiff error: {err}");
                    std::process::exit(2);
                }
            }
        }
        _ => {
            eprintln!("RustQR scaffold CLI");
            eprintln!("usage: qrtool smoke");
            eprintln!(
                "       qrtool reading-rate [--profile boofcv-all|boofcv-<category>|payload-validated] [--dataset-root PATH] [--artifact PATH] [--limit N] [--max-working-dim N] [--emergency-cutoff-ms N] [--gate-global-rate-min F64] [--gate-rotations-rate-min F64] [--gate-high-version-rate-min F64] [--gate-median-runtime-ms-max F64]"
            );
            eprintln!("       qrtool benchdiff --base PATH --candidate PATH [--artifact PATH]");
            std::process::exit(2);
        }
    }
}
