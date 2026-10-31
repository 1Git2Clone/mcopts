use std::{fmt, io, path::PathBuf};

use crate::{Problem, versions};

#[derive(Debug)]
pub enum Error {
    Io(PathBuf, io::Error),
    /// The game would drop these, or discard the whole file.
    Invalid(Vec<Problem>),
    UnknownVersion(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(path, e) => write!(f, "{}: {e}", path.display()),
            Error::Invalid(problems) => {
                problems.iter().try_for_each(|p| writeln!(f, "{p}"))?;
                f.write_str("Minecraft would drop these, or discard the whole options.txt")
            }
            Error::UnknownVersion(name) => {
                let known: Vec<_> = versions().map(|v| v.name).collect();
                write!(
                    f,
                    "Minecraft {name} is not supported; known: {}",
                    known.join(", ")
                )
            }
        }
    }
}

impl std::error::Error for Error {}
