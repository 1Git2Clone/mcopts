use std::{fs, path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};
use mcopts::{Options, Version};

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

fn version(name: &str) -> Result<Version, String> {
    mcopts::version(name).ok_or_else(|| {
        let known: Vec<_> = mcopts::versions().map(|v| v.name).collect();
        format!(
            "Minecraft {name} is not supported; known: {}",
            known.join(", ")
        )
    })
}

fn read(path: &PathBuf) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn run(cli: Cli) -> Result<bool, String> {
    match cli.command {
        Command::Versions => {
            for v in mcopts::versions() {
                println!("{}\t{}\t{}", v.name, v.data_version, v.defaults.len());
            }
            Ok(true)
        }
        Command::Check { minecraft, file } => {
            let problems = mcopts::check(&read(&file)?, &version(&minecraft)?);
            for p in &problems {
                eprintln!(
                    "{}: {}{p}",
                    file.display(),
                    if p.warning { "warning: " } else { "" }
                );
            }
            let failed = problems.iter().any(|p| !p.warning);
            if failed {
                eprintln!("Minecraft would drop these, or discard the whole options.txt.");
            }
            Ok(!failed)
        }
        Command::Apply {
            minecraft,
            out,
            sources,
        } => {
            let notes =
                mcopts::apply(&out, &sources, &version(&minecraft)?).map_err(|e| e.to_string())?;
            notes.iter().for_each(|n| {
                if n.starts_with("warning:") {
                    eprintln!("{n}");
                } else {
                    eprintln!("note: {n}");
                }
            });
            Ok(true)
        }
        Command::Sync {
            instance,
            target,
            except,
        } => {
            let synced = mcopts::sync(&instance, &target, &except).map_err(|e| e.to_string())?;
            println!(
                "{} options, {} config files",
                synced.options,
                synced.configs.len()
            );
            synced
                .configs
                .iter()
                .for_each(|c| println!("  {}", c.display()));
            Ok(true)
        }
        Command::Diff { a, b } => {
            let (a, b) = (Options::parse(&read(&a)?), Options::parse(&read(&b)?));
            let diff = mcopts::diff(&a, &b);
            for (key, ours, theirs) in &diff {
                println!("{key}: {ours} -> {}", theirs.unwrap_or("(missing)"));
            }
            Ok(diff.is_empty())
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("mcopts: {e}");
            ExitCode::FAILURE
        }
    }
}
