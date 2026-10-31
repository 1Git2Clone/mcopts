//! A library for managing your Minecraft options: read, check, merge and sync
//! `options.txt` and mod configs.
//!
//! With the tested modern keybinds, a missing `version:` makes every supported
//! client log `Failed to load options` and fall back to defaults. This is not
//! a claim that every versionless file fails: legacy files can be migrated.
//! Unknown keys are dropped on save; mod keybinds need their mods installed.
//!
//! Every supported version's defaults, as the game itself writes them, are
//! embedded from `data/defaults/<version>.txt`. They give each version's data
//! version, its options, and what kind of value each takes.

use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

/// `options.txt` as key/value pairs, in file order, each key once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options(Vec<(String, String)>);

impl Options {
    /// Lines without a `:` and lines starting with `#` are skipped, so source
    /// files can carry comments. A repeated key keeps its first position and
    /// its last value.
    pub fn parse(text: &str) -> Self {
        let mut options = Options::default();
        for line in text.lines().filter(|l| !l.starts_with('#')) {
            if let Some((k, v)) = line.split_once(':') {
                options.set(k, v);
            }
        }
        options
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Replaces the value in place, or appends the key.
    pub fn set(&mut self, key: &str, value: &str) {
        match self.0.iter_mut().find(|(k, _)| k == key) {
            Some((_, v)) => *v = value.to_owned(),
            None => self.0.push((key.to_owned(), value.to_owned())),
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        let i = self.0.iter().position(|(k, _)| k == key)?;
        Some(self.0.remove(i).1)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for Options {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.iter().try_for_each(|(k, v)| writeln!(f, "{k}:{v}"))
    }
}

static DEFAULTS: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/defaults.rs"));

/// A supported Minecraft version: its data version and its default options.
#[derive(Clone, Debug)]
pub struct Version {
    pub name: &'static str,
    /// The `version:` options.txt must carry (version.json's `world_version`).
    pub data_version: u32,
    pub defaults: Options,
}

impl Version {
    fn new(name: &'static str, text: &str) -> Self {
        let defaults = Options::parse(text);
        let data_version = defaults
            .get("version")
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| panic!("data/defaults/{name}.txt has no version:"));
        Version {
            name,
            data_version,
            defaults,
        }
    }
}

/// Every supported version, oldest first.
pub fn versions() -> impl Iterator<Item = Version> {
    DEFAULTS.iter().map(|(name, text)| Version::new(name, text))
}

pub fn version(name: &str) -> Option<Version> {
    versions().find(|v| v.name == name)
}

/// The kind of value an option takes, read off its default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Bool,
    Number,
    List,
    Quoted,
    Key,
    Text,
}

impl Kind {
    fn of(key: &str, default: &str) -> Kind {
        if key.starts_with("key_") {
            Kind::Key
        } else if default == "true" || default == "false" {
            Kind::Bool
        } else if default.parse::<f64>().is_ok() {
            Kind::Number
        } else if default.starts_with('[') {
            Kind::List
        } else if default.starts_with('"') {
            Kind::Quoted
        } else {
            Kind::Text
        }
    }

    fn fits(self, value: &str) -> bool {
        match self {
            Kind::Bool => value == "true" || value == "false",
            Kind::Number => value.parse::<f64>().is_ok(),
            Kind::List => value.starts_with('[') && value.ends_with(']'),
            Kind::Quoted => value.len() >= 2 && value.starts_with('"') && value.ends_with('"'),
            Kind::Key => is_key_name(value),
            Kind::Text => true,
        }
    }

    fn describe(self) -> &'static str {
        match self {
            Kind::Bool => "true or false",
            Kind::Number => "a number",
            Kind::List => "a [...] list",
            Kind::Quoted => "a \"quoted\" string",
            Kind::Key => "a key.keyboard.* or key.mouse.* name",
            Kind::Text => "text",
        }
    }
}

/// `key.keyboard.b`, `key.mouse.left`: what keybinds have been since 1.13.
/// A bare number is a pre-1.13 LWJGL2 keycode.
fn is_key_name(value: &str) -> bool {
    ["key.keyboard.", "key.mouse."].iter().any(|p| {
        value.strip_prefix(p).is_some_and(|rest| {
            !rest.is_empty()
                && rest
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c))
        })
    })
}

/// A validation error or advisory warning about options.txt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// 1-based, where the problem is on one line.
    pub line: Option<usize>,
    pub message: String,
    /// Advisory only; callers should fail only on problems without this flag.
    pub warning: bool,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.line {
            Some(n) => write!(f, "line {n}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

/// Problems with `text` as options.txt: the file may fail to load
/// (missing or mismatched `version:`), or the game may drop an entry
/// (an option it does not have, a value of the wrong kind). Keybinds it does
/// not know are allowed, as mods add their own, but must be key names. Vanilla
/// drops those on save; this check does not prove that a mod is installed.
/// Value kinds do not detect every range, enum or graphics-preset conflict.
/// Entries with [`Problem::warning`] are advisory and must not fail a check.
pub fn check(text: &str, version: &Version) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut seen: Vec<(&str, usize)> = Vec::new();
    let mut problem = |line, message: String| {
        problems.push(Problem {
            line,
            message,
            warning: false,
        })
    };

    for (n, line) in (1..).zip(text.lines()) {
        let Some((key, value)) = line.split_once(':') else {
            problem(Some(n), format!("not key:value: {line:?}"));
            continue;
        };
        if let Some((_, first)) = seen.iter().find(|(k, _)| *k == key) {
            problem(Some(n), format!("{key} again (first on line {first})"));
        }
        seen.push((key, n));
        if key == "version" {
            if value != version.data_version.to_string() {
                problem(
                    Some(n),
                    format!(
                        "version:{value}, but Minecraft {}'s data version is {}",
                        version.name, version.data_version
                    ),
                );
            }
            continue;
        }
        let kind = match version.defaults.get(key) {
            Some(default) => Kind::of(key, default),
            None if key.starts_with("key_") => Kind::Key,
            None => {
                problem(
                    Some(n),
                    format!(
                        "Minecraft {} has no option {key}; the game drops it",
                        version.name
                    ),
                );
                continue;
            }
        };
        if !kind.fits(value) {
            problem(
                Some(n),
                format!("{key} is {value:?}, not {}", kind.describe()),
            );
        }
    }
    if !seen.iter().any(|(k, _)| *k == "version") {
        problem(
            None,
            format!(
                "no version: line; modern keybinds can make Minecraft fail to load the file \
                 (want version:{})",
                version.data_version
            ),
        );
    }
    // Seen black-box: a preset other than "custom" overwrites these on load.
    const PRESET_OVERRIDES: &[&str] = &["simulationDistance", "entityDistanceScaling"];
    let options = Options::parse(text);
    if let Some(preset) = options
        .get("graphicsPreset")
        .filter(|p| *p != "\"custom\"" && version.defaults.get("graphicsPreset").is_some())
    {
        for key in PRESET_OVERRIDES.iter().filter(|k| options.get(k).is_some()) {
            problems.push(Problem {
                line: None,
                message: format!("graphicsPreset:{preset} may override {key}; use graphicsPreset:\"custom\" to keep individual settings"),
                warning: true,
            });
        }
    }
    problems
}

/// Lays `layers` over `base`, key by key, into an options.txt for `version`,
/// with its `version:` first.
///
/// One set of options serves several versions, so an option this version
/// lacks, or a value of a kind this version does not take, is left out and
/// listed in the returned notes. One that NO supported version would take is
/// a mistake, a typo or a pre-1.13 keybind, and fails the merge.
pub fn merge(
    base: &Options,
    layers: &[Options],
    version: &Version,
) -> Result<(Options, Vec<String>), Vec<Problem>> {
    let mut merged = base.clone();
    for layer in layers {
        layer.iter().for_each(|(k, v)| merged.set(k, v));
    }
    merged.remove("version");

    let all: Vec<Version> = versions().collect();
    let fits_any = |key: &str, value: &str| {
        all.iter().any(|v| {
            v.defaults
                .get(key)
                .is_some_and(|d| Kind::of(key, d).fits(value))
        })
    };

    let mut out = Options::default();
    out.set("version", &version.data_version.to_string());
    let (mut notes, mut problems) = (Vec::new(), Vec::new());
    for (key, value) in merged.iter() {
        let fits_here = match version.defaults.get(key) {
            Some(default) => Kind::of(key, default).fits(value),
            None => key.starts_with("key_") && is_key_name(value),
        };
        if fits_here {
            out.set(key, value);
        } else if fits_any(key, value) {
            notes.push(format!(
                "{key}:{value} left out: Minecraft {} does not take it",
                version.name
            ));
        } else {
            problems.push(Problem {
                line: None,
                message: format!("{key}:{value} is not an option any supported Minecraft takes"),
                warning: false,
            });
        }
    }
    if problems.is_empty() {
        Ok((out, notes))
    } else {
        Err(problems)
    }
}

static NOT_PREFERENCES: &str = include_str!("../data/not-preferences.txt");

/// Whether a file under the game folder (`config/...`) holds a player's
/// preferences, rather than mod internals, server or world settings, or a
/// cache. See data/not-preferences.txt.
pub fn is_preference(path: &str) -> bool {
    !NOT_PREFERENCES
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .any(|pattern| path.contains(pattern))
}

/// Options whose value in `a` is not the one in `b`: missing there, or
/// different. Numbers compare by value, so `1` matches `1.0`.
pub fn diff<'a>(a: &'a Options, b: &'a Options) -> Vec<(&'a str, &'a str, Option<&'a str>)> {
    let same = |x: &str, y: &str| {
        x == y || matches!((x.parse::<f64>(), y.parse::<f64>()), (Ok(x), Ok(y)) if x == y)
    };
    a.iter()
        .filter(|(k, v)| !b.get(k).is_some_and(|w| same(v, w)))
        .map(|(k, v)| (k, v, b.get(k)))
        .collect()
}

#[derive(Debug)]
pub enum Error {
    Io(PathBuf, io::Error),
    Invalid(Vec<Problem>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(path, e) => write!(f, "{}: {e}", path.display()),
            Error::Invalid(problems) => {
                problems.iter().try_for_each(|p| writeln!(f, "{p}"))?;
                f.write_str("Minecraft would drop these, or discard the whole options.txt")
            }
        }
    }
}

impl std::error::Error for Error {}

fn read(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|e| Error::Io(path.into(), e))
}

fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), Error> {
    let io = |e| Error::Io(path.into(), e);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io)?;
    }
    fs::write(path, contents).map_err(io)
}

/// Every file under `dir`, relative to it, sorted.
fn files(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut out = Vec::new();
    let mut todo = vec![PathBuf::new()];
    while let Some(rel) = todo.pop() {
        let entries = fs::read_dir(dir.join(&rel)).map_err(|e| Error::Io(dir.join(&rel), e))?;
        for entry in entries {
            let entry = entry.map_err(|e| Error::Io(dir.join(&rel), e))?;
            let path = rel.join(entry.file_name());
            if entry.path().is_dir() {
                todo.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Lays option directories, each mirroring the game folder, over `out` in
/// order: `options.txt` merges key by key (see [`merge`]) over `out`'s own,
/// and every other file replaces `out`'s whole. Returns the merge's notes.
pub fn apply(out: &Path, sources: &[PathBuf], version: &Version) -> Result<Vec<String>, Error> {
    let target = out.join("options.txt");
    let base = match target.exists() {
        true => Options::parse(&read(&target)?),
        false => Options::default(),
    };
    let mut layers = Vec::new();
    for src in sources {
        for rel in files(src)? {
            if rel == Path::new("options.txt") {
                layers.push(Options::parse(&read(&src.join(&rel))?));
            } else {
                let from = src.join(&rel);
                write(
                    &out.join(&rel),
                    fs::read(&from).map_err(|e| Error::Io(from, e))?,
                )?;
            }
        }
    }
    if base.is_empty() && layers.is_empty() {
        return Ok(Vec::new());
    }
    let (merged, mut notes) = merge(&base, &layers, version).map_err(Error::Invalid)?;
    let text = merged.to_string();
    let mut problems = check(&text, version);
    notes.extend(
        problems
            .iter()
            .filter(|p| p.warning)
            .map(|p| format!("warning: {p}")),
    );
    problems.retain(|p| !p.warning);
    if !problems.is_empty() {
        return Err(Error::Invalid(problems));
    }
    write(&target, text)?;
    Ok(notes)
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
                let from = instance.join(&rel);
                write(
                    &target.join(&rel),
                    fs::read(&from).map_err(|e| Error::Io(from, e))?,
                )?;
                synced.configs.push(rel);
            }
        }
    }
    Ok(synced)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn latest() -> Version {
        versions().last().unwrap()
    }

    fn valid(v: &Version) -> String {
        format!(
            "version:{}\nfov:1.0\nkey_key.jump:key.keyboard.space\n",
            v.data_version
        )
    }

    #[test]
    fn parse_skips_comments_and_keeps_colons_in_values() {
        let o =
            Options::parse("# a: comment\nresourcePacks:[\"punchy:punchy\"]\nfov:0.5\nfov:1.0\n");
        assert_eq!(
            o.to_string(),
            "resourcePacks:[\"punchy:punchy\"]\nfov:1.0\n"
        );
    }

    #[test]
    fn data_versions_match_the_client_jars() {
        // world_version in each client jar's version.json.
        for (name, dv) in [
            ("1.21.1", 3955),
            ("26.1.2", 4790),
            ("26.2", 4903),
            ("26.3", 5023),
        ] {
            assert_eq!(version(name).unwrap().data_version, dv, "{name}");
        }
    }

    #[test]
    fn check_accepts_a_good_file() {
        let v = latest();
        assert_eq!(check(&valid(&v), &v), vec![]);
    }

    #[test]
    fn check_warns_about_observed_preset_overrides() {
        for name in ["1.21.11", "26.1", "26.1.1", "26.1.2", "26.2", "26.3"] {
            let v = version(name).unwrap();
            let text = valid(&v)
                + "graphicsPreset:\"fancy\"\nsimulationDistance:6\nentityDistanceScaling:0.75\n";
            let problems = check(&text, &v);
            assert_eq!(problems.len(), 2, "{name}");
            assert!(problems.iter().all(|p| p.warning));
            assert!(check(&text.replace("\"fancy\"", "\"custom\""), &v).is_empty());
            assert!(check(&(valid(&v) + "graphicsPreset:\"fancy\"\n"), &v).is_empty());
        }
        let old = version("1.21.10").unwrap();
        assert!(
            check(
                &(valid(&old) + "simulationDistance:6\nentityDistanceScaling:0.75\n"),
                &old
            )
            .is_empty()
        );
    }

    #[test]
    fn check_catches_what_the_game_drops() {
        let v = latest();
        let one = |text: &str| {
            let p = check(text, &v);
            assert_eq!(p.len(), 1, "{text:?}: {p:?}");
            p[0].message.clone()
        };
        assert!(one("fov:1.0\n").contains("no version"));
        assert!(one(&valid(&v).replace(&v.data_version.to_string(), "1")).contains("data version"));
        assert!(one(&(valid(&v) + "key_key.use:57\n")).contains("key.keyboard"));
        assert!(one(&(valid(&v) + "renderDistanse:12\n")).contains("no option"));
        assert!(one(&(valid(&v) + "fov:0.5\n")).contains("again"));
        assert!(one(&(valid(&v) + "bobView:yes\n")).contains("true or false"));
        assert!(one(&(valid(&v) + "# note\n")).contains("not key:value"));
        // A mod's keybind is fine, as long as it is a key name.
        assert_eq!(
            check(&(valid(&v) + "key_key.mymod.open:key.keyboard.b\n"), &v),
            vec![]
        );
    }

    #[test]
    fn merge_stamps_the_version_and_drops_only_other_versions_options() {
        let v = latest();
        let base = Options::parse("fov:0.0\nmaxFps:60\n");
        let layer = Options::parse("version:1\nfov:1.0\nkey_key.mymod.open:key.keyboard.b\n");
        let (out, notes) = merge(&base, &[layer], &v).unwrap();
        assert_eq!(
            out.iter().next(),
            Some(("version", v.data_version.to_string().as_str()))
        );
        assert_eq!(out.get("fov"), Some("1.0"));
        assert_eq!(out.get("maxFps"), Some("60"));
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(check(&out.to_string(), &v), vec![]);

        let typo = Options::parse("renderDistanse:12\n");
        assert!(merge(&Options::default(), &[typo], &v).is_err());
        let lwjgl2 = Options::parse("key_key.use:57\n");
        assert!(merge(&Options::default(), &[lwjgl2], &v).is_err());
    }

    #[test]
    fn merge_leaves_out_an_option_only_newer_versions_have() {
        let (old, new) = (versions().next().unwrap(), latest());
        let Some((key, value)) = new
            .defaults
            .iter()
            .find(|(k, _)| old.defaults.get(k).is_none())
        else {
            return;
        };
        let (out, notes) = merge(
            &Options::default(),
            &[Options::parse(&format!("{key}:{value}"))],
            &old,
        )
        .unwrap();
        assert_eq!(out.get(key), None);
        assert_eq!(notes.len(), 1);
    }

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
    fn diff_compares_numbers_by_value() {
        let a = Options::parse("fov:1\nguiScale:2\n");
        let b = Options::parse("fov:1.0\nguiScale:0\n");
        assert_eq!(diff(&a, &b), vec![("guiScale", "2", Some("0"))]);
    }
}
