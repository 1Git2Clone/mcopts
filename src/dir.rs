//! Option directories, each mirroring the game folder: [`apply`] lays them
//! over a game folder, [`sync`] takes an instance's settings into one.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{Error, Options, Problem, Version, merge};

/// Reads a file, naming it in the error.
pub fn read(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|e| Error::Io(path.into(), e))
}

fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), Error> {
    let io = |e| Error::Io(path.into(), e);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io)?;
    }
    fs::write(path, contents).map_err(io)
}

fn copy(from: &Path, to: &Path) -> Result<(), Error> {
    write(to, fs::read(from).map_err(|e| Error::Io(from.into(), e))?)
}

/// Every file under `dir`, relative to it, sorted.
fn files(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut out = Vec::new();
    let mut todo = vec![PathBuf::new()];
    while let Some(rel) = todo.pop() {
        let io = |e| Error::Io(dir.join(&rel), e);
        for entry in fs::read_dir(dir.join(&rel)).map_err(io)? {
            let entry = entry.map_err(io)?;
            let path = rel.join(entry.file_name());
            match entry.path().is_dir() {
                true => todo.push(path),
                false => out.push(path),
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Lays option directories, each mirroring the game folder, over `out` in
/// order: `options.txt` merges key by key (see [`merge`]) over `out`'s own,
/// and every other file replaces `out`'s whole. Returns the merge's notes and
/// the result's warnings; anything the game would not load fails it, and
/// leaves `out`'s options.txt as it was.
pub fn apply(
    out: &Path,
    sources: &[PathBuf],
    version: &'static Version,
) -> Result<Vec<Problem>, Error> {
    let target = out.join("options.txt");
    let base = match target.exists() {
        true => Options::parse(&read(&target)?),
        false => Options::default(),
    };
    let mut layers = Vec::new();
    for src in sources {
        for rel in files(src)? {
            match rel == Path::new("options.txt") {
                true => layers.push(Options::parse(&read(&src.join(&rel))?)),
                false => copy(&src.join(&rel), &out.join(&rel))?,
            }
        }
    }
    if base.is_empty() && layers.is_empty() {
        return Ok(Vec::new());
    }
    let (checked, mut problems) = merge(&base, &layers, version).map_err(Error::Invalid)?;
    write(&target, checked.to_string())?;
    problems.extend_from_slice(checked.warnings());
    Ok(problems)
}

/// Keys `sync` never takes from an instance: the data version (written per
/// version by `apply`), resource packs (which a pack ships or does not), and
/// the last server typed into Direct Connect.
pub const NOT_SYNCED: &[&str] = &[
    "version",
    "resourcePacks",
    "incompatibleResourcePacks",
    "lastServer",
];

static NOT_PREFERENCES: &str = include_str!("../data/not-preferences.txt");

/// Whether a file under the game folder (`config/...`) holds a player's
/// preferences, rather than a mod's internals, the server's or the world's
/// settings, or a cache. See data/not-preferences.txt.
pub fn is_preference(path: &str) -> bool {
    !NOT_PREFERENCES
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .any(|pattern| path.contains(pattern))
}

/// What [`sync`] took from an instance.
#[derive(Debug, Default)]
pub struct Synced {
    pub options: usize,
    pub configs: Vec<PathBuf>,
}

/// Takes a game instance's settings into an option directory: its options
/// (bar [`NOT_SYNCED`] and `except`) over the directory's options.txt, whose
/// leading comments and other keys stay, and every config file that
/// [`is_preference`]. Files the instance lacks are left alone.
pub fn sync(instance: &Path, target: &Path, except: &[String]) -> Result<Synced, Error> {
    let mut synced = Synced::default();
    let from = instance.join("options.txt");
    if from.exists() {
        let mut theirs = Options::parse(&read(&from)?);
        for key in NOT_SYNCED
            .iter()
            .copied()
            .chain(except.iter().map(String::as_str))
        {
            theirs.remove(key);
        }
        let to = target.join("options.txt");
        let ours = match to.exists() {
            true => read(&to)?,
            false => String::new(),
        };
        let header: String = ours
            .lines()
            .take_while(|l| l.starts_with('#') || l.trim().is_empty())
            .map(|l| format!("{l}\n"))
            .collect();
        let mut merged = theirs.clone();
        for (k, v) in Options::parse(&ours).iter() {
            if theirs.get(k).is_none() {
                merged.set(k, v);
            }
        }
        synced.options = theirs.len();
        write(&to, header + &merged.to_string())?;
    }
    let config = instance.join("config");
    if config.exists() {
        for rel in files(&config)? {
            let rel = Path::new("config").join(rel);
            if is_preference(&rel.to_string_lossy()) {
                copy(&instance.join(&rel), &target.join(&rel))?;
                synced.configs.push(rel);
            }
        }
    }
    Ok(synced)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences() {
        for p in [
            "config/sodium-options.json",
            "config/xaero/minimap/profiles/default.cfg",
            "config/jade/jade.json",
        ] {
            assert!(is_preference(p), "{p}");
        }
        for p in [
            "config/lithium.properties",
            "config/sodium-mixins.properties",
            "config/comforts-server.toml",
            "config/xaero/minimap/common.cfg",
            "config/wilderwild/worldgen.json5",
            "config/yosbr/options.txt",
            "config/shulkerboxtooltip_mp_ender_chest_cache.dat",
        ] {
            assert!(!is_preference(p), "{p}");
        }
    }

    #[test]
    fn read_names_the_missing_file() {
        let e = read(Path::new("/nonexistent/options.txt"))
            .unwrap_err()
            .to_string();
        assert!(e.starts_with("/nonexistent/options.txt: "), "{e}");
    }
}
