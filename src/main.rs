use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};
use mcopts::{Checked, Error, Options};

/// Read, check and sync Minecraft options.txt and mod configs.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List the supported Minecraft versions: name, data version, options.
    Versions,
    /// Fail on anything in an options.txt that Minecraft would drop, or that
    /// would make it discard the whole file.
    Check {
        #[arg(short, long)]
        minecraft: String,
        file: PathBuf,
    },
    /// Lay option directories (each mirroring the game folder) over OUT, in
    /// order: options.txt merges key by key and gets the version's data
    /// version; every other file replaces OUT's.
    Apply {
        #[arg(short, long)]
        minecraft: String,
        out: PathBuf,
        #[arg(required = true)]
        sources: Vec<PathBuf>,
    },
    /// Take an instance's options and preference configs into an option
    /// directory.
    Sync {
        instance: PathBuf,
        target: PathBuf,
        /// An option to keep as the target has it (repeatable).
        #[arg(long)]
        except: Vec<String>,
    },
    /// List options in A whose value B lacks or differs from.
    Diff { a: PathBuf, b: PathBuf },
}

fn run(cli: Cli) -> Result<ExitCode, Error> {
    let failed_if = |failed: bool| match failed {
        true => ExitCode::FAILURE,
        false => ExitCode::SUCCESS,
    };
    match cli.command {
        Command::Versions => {
            for v in mcopts::versions() {
                println!("{}\t{}\t{}", v.name, v.data_version, v.defaults.len());
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Check { minecraft, file } => {
            let version = mcopts::version(&minecraft)?;
            let (problems, failed) = match Checked::parse(&mcopts::read(&file)?, version) {
                Ok(checked) => (checked.warnings().to_vec(), false),
                Err(problems) => (problems, true),
            };
            for p in &problems {
                eprintln!("{}: {p}", file.display());
            }
            if failed {
                eprintln!("Minecraft would drop these, or discard the whole options.txt.");
            }
            Ok(failed_if(failed))
        }
        Command::Apply {
            minecraft,
            out,
            sources,
        } => {
            for p in mcopts::apply(&out, &sources, mcopts::version(&minecraft)?)? {
                eprintln!("{p}");
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Sync {
            instance,
            target,
            except,
        } => {
            let synced = mcopts::sync(&instance, &target, &except)?;
            println!(
                "{} options, {} config files",
                synced.options,
                synced.configs.len()
            );
            for c in &synced.configs {
                println!("  {}", c.display());
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Diff { a, b } => {
            let (a, b) = (
                Options::parse(&mcopts::read(&a)?),
                Options::parse(&mcopts::read(&b)?),
            );
            let diff = mcopts::diff(&a, &b);
            for (key, ours, theirs) in &diff {
                println!("{key}: {ours} -> {}", theirs.unwrap_or("(missing)"));
            }
            Ok(failed_if(!diff.is_empty()))
        }
    }
}

fn main() -> ExitCode {
    run(Cli::parse()).unwrap_or_else(|e| {
        eprintln!("mcopts: {e}");
        ExitCode::FAILURE
    })
}
