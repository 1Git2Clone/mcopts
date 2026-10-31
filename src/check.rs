//! [`Checked`]: options the game will load, and the [`Problem`]s found getting there.

use std::fmt;

use crate::{Options, Value, Version};

/// How much a [`Problem`] matters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// For the record: an option left out for a version that lacks it.
    Note,
    /// Loads, but likely not as meant.
    Warning,
    /// The game would drop it, or discard the whole file.
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Severity::Note => "note",
            Severity::Warning => "warning",
            Severity::Error => "error",
        })
    }
}

/// Something about an options.txt worth telling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub severity: Severity,
    /// 1-based, where the problem is on one line.
    pub line: Option<usize>,
    pub message: String,
}

impl Problem {
    pub(crate) fn new(severity: Severity, line: Option<usize>, message: String) -> Self {
        Problem {
            severity,
            line,
            message,
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: ", self.severity)?;
        match self.line {
            Some(n) => write!(f, "line {n}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

/// Options a version of the game will load as written: every key one it
/// keeps, every value of the kind it takes, and its `version:`. The only ways
/// to get one are [`Checked::parse`], [`Checked::new`] and [`crate::merge`],
/// so holding one is the proof. Displays as the options.txt to write, with
/// `version:` first.
///
/// Value kinds come from each default's shape, so ranges, enum members and
/// interactions between settings are not checked, bar the graphics preset
/// overrides seen in the real game (a warning). Mod keybinds are kept as long
/// as they are key names; vanilla drops them on save.
#[derive(Clone, Debug)]
pub struct Checked {
    version: &'static Version,
    entries: Vec<(String, Value)>,
    warnings: Vec<Problem>,
}

impl Checked {
    /// Checks an options.txt as the game would read it. On failure, every
    /// problem found, warnings included.
    pub fn parse(text: &str, version: &'static Version) -> Result<Self, Vec<Problem>> {
        let mut problems = Vec::new();
        let mut entries = Vec::new();
        let mut seen: Vec<(&str, usize)> = Vec::new();
        for (n, line) in (1..).zip(text.lines()) {
            let Some((key, value)) = line.split_once(':') else {
                problems.push(Problem::new(
                    Severity::Error,
                    Some(n),
                    format!("not key:value: {line:?}"),
                ));
                continue;
            };
            if let Some((_, first)) = seen.iter().find(|(k, _)| *k == key) {
                let message = format!("{key} again (first on line {first})");
                problems.push(Problem::new(Severity::Error, Some(n), message));
                continue;
            }
            seen.push((key, n));
            match entry(version, key, value) {
                Ok(Some(value)) => entries.push((key.to_owned(), value)),
                Ok(None) => {}
                Err(message) => problems.push(Problem::new(Severity::Error, Some(n), message)),
            }
        }
        if !seen.iter().any(|(k, _)| *k == "version") {
            let message = format!(
                "no version: line; modern keybinds can make Minecraft fail to load the file \
                 (want version:{})",
                version.data_version
            );
            problems.push(Problem::new(Severity::Error, None, message));
        }
        Self::finish(version, entries, problems)
    }

    /// Checks options built in code, such as a merge's. A `version:` among
    /// them must be this version's; none is fine, as one is always written.
    pub fn new(options: &Options, version: &'static Version) -> Result<Self, Vec<Problem>> {
        let mut problems = Vec::new();
        let mut entries = Vec::new();
        for (key, value) in options.iter() {
            match entry(version, key, value) {
                Ok(Some(value)) => entries.push((key.to_owned(), value)),
                Ok(None) => {}
                Err(message) => problems.push(Problem::new(Severity::Error, None, message)),
            }
        }
        Self::finish(version, entries, problems)
    }

    fn finish(
        version: &'static Version,
        entries: Vec<(String, Value)>,
        mut problems: Vec<Problem>,
    ) -> Result<Self, Vec<Problem>> {
        let checked = Checked {
            version,
            entries,
            warnings: Vec::new(),
        };
        problems.extend(checked.preset_overrides());
        if problems.iter().any(|p| p.severity == Severity::Error) {
            return Err(problems);
        }
        Ok(Checked {
            warnings: problems,
            ..checked
        })
    }

    /// Seen black-box from 1.21.11 on: a graphics preset other than "custom"
    /// overwrites these on load.
    fn preset_overrides(&self) -> Vec<Problem> {
        const OVERRIDDEN: &[&str] = &["simulationDistance", "entityDistanceScaling"];
        let Some(preset) = self
            .get("graphicsPreset")
            .filter(|p| p.to_string() != "\"custom\"")
        else {
            return Vec::new();
        };
        OVERRIDDEN
            .iter()
            .filter(|k| self.get(k).is_some())
            .map(|k| {
                let message = format!(
                    "graphicsPreset:{preset} may override {k}; use graphicsPreset:\"custom\" \
                     to keep individual settings"
                );
                Problem::new(Severity::Warning, None, message)
            })
            .collect()
    }

    pub fn version(&self) -> &'static Version {
        self.version
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Every option bar `version:`, in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// What loads, but likely not as meant.
    pub fn warnings(&self) -> &[Problem] {
        &self.warnings
    }
}

impl fmt::Display for Checked {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "version:{}", self.version.data_version)?;
        self.iter().try_for_each(|(k, v)| writeln!(f, "{k}:{v}"))
    }
}

/// One entry checked against `version`: its value, None for `version:` itself
/// (always written from the version), or why the game would not keep it.
fn entry(version: &Version, key: &str, value: &str) -> Result<Option<Value>, String> {
    if key == "version" {
        return match value == version.data_version.to_string() {
            true => Ok(None),
            false => Err(format!(
                "version:{value}, but Minecraft {}'s data version is {}",
                version.name, version.data_version
            )),
        };
    }
    let kind = version.kind(key).ok_or_else(|| {
        format!(
            "Minecraft {} has no option {key}; the game drops it",
            version.name
        )
    })?;
    kind.parse(value)
        .map(Some)
        .ok_or_else(|| format!("{key} is {value:?}, not {}", kind.describe()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{version, versions};

    fn latest() -> &'static Version {
        versions().last().unwrap()
    }

    fn valid(v: &Version) -> String {
        format!(
            "version:{}\nfov:1.0\nkey_key.jump:key.keyboard.space\n",
            v.data_version
        )
    }

    fn errors(text: &str, v: &'static Version) -> Vec<String> {
        Checked::parse(text, v)
            .unwrap_err()
            .into_iter()
            .filter(|p| p.severity == Severity::Error)
            .map(|p| p.message)
            .collect()
    }

    #[test]
    fn a_good_file_round_trips() {
        let v = latest();
        let c = Checked::parse(&valid(v), v).unwrap();
        assert!(c.warnings().is_empty());
        assert_eq!(c.to_string(), valid(v));
        assert_eq!(c.version().name, v.name);
        assert_eq!(c.get("fov").unwrap().to_string(), "1.0");
        assert_eq!(c.iter().count(), 2);
    }

    #[test]
    fn every_version_takes_its_own_defaults() {
        for v in versions() {
            let c = Checked::new(&v.defaults, v).unwrap_or_else(|p| panic!("{}: {p:?}", v.name));
            assert_eq!(c.iter().count(), v.defaults.len() - 1, "{}", v.name);
        }
    }

    #[test]
    fn parse_catches_what_the_game_drops_or_chokes_on() {
        let v = latest();
        let one = |text: &str| {
            let e = errors(text, v);
            assert_eq!(e.len(), 1, "{text:?}: {e:?}");
            e[0].clone()
        };
        assert!(one("fov:1.0\n").contains("no version"));
        assert!(one(&valid(v).replace(&v.data_version.to_string(), "1")).contains("data version"));
        assert!(one(&(valid(v) + "key_key.use:57\n")).contains("key.keyboard"));
        assert!(one(&(valid(v) + "renderDistanse:12\n")).contains("no option"));
        assert!(one(&(valid(v) + "fov:0.5\n")).contains("again"));
        assert!(one(&(valid(v) + "bobView:yes\n")).contains("true or false"));
        assert!(one(&(valid(v) + "# note\n")).contains("not key:value"));
    }

    #[test]
    fn errors_carry_their_line() {
        let v = latest();
        let p = Checked::parse(&(valid(v) + "bobView:yes\n"), v).unwrap_err();
        assert_eq!(p[0].line, Some(4));
        assert!(p[0].to_string().starts_with("error: line 4: "), "{}", p[0]);
    }

    #[test]
    fn a_mod_keybind_is_kept_if_it_is_a_key_name() {
        let v = latest();
        let c = Checked::parse(&(valid(v) + "key_key.mymod.open:key.keyboard.b\n"), v).unwrap();
        assert!(matches!(c.get("key_key.mymod.open"), Some(Value::Key(_))));
    }

    #[test]
    fn new_writes_the_version_and_rejects_a_wrong_one() {
        let v = latest();
        let c = Checked::new(&Options::parse("fov:1.0\n"), v).unwrap();
        assert_eq!(
            c.to_string(),
            format!("version:{}\nfov:1.0\n", v.data_version)
        );
        assert!(Checked::new(&Options::parse("version:1\n"), v).is_err());
        assert!(Checked::new(&Options::parse("renderDistanse:12\n"), v).is_err());
    }

    #[test]
    fn warns_about_observed_preset_overrides() {
        let with_preset: Vec<_> = versions()
            .filter(|v| v.defaults.get("graphicsPreset").is_some())
            .collect();
        assert_eq!(with_preset.len(), 6);
        for v in with_preset {
            let text = valid(v)
                + "graphicsPreset:\"fancy\"\nsimulationDistance:6\nentityDistanceScaling:0.75\n";
            let c = Checked::parse(&text, v).unwrap();
            assert_eq!(c.warnings().len(), 2, "{}", v.name);
            assert!(c.warnings().iter().all(|p| p.severity == Severity::Warning));
            let custom = Checked::parse(&text.replace("\"fancy\"", "\"custom\""), v).unwrap();
            assert!(custom.warnings().is_empty());
        }
        // An error alongside: the warnings come with it.
        let v = latest();
        let bad = valid(v) + "graphicsPreset:\"fancy\"\nsimulationDistance:6\nbobView:yes\n";
        let p = Checked::parse(&bad, v).unwrap_err();
        assert_eq!(
            p.iter().filter(|p| p.severity == Severity::Warning).count(),
            1
        );
        // Before presets, the same options are just options.
        let old = version("1.21.10").unwrap();
        let c = Checked::parse(&(valid(old) + "simulationDistance:6\n"), old).unwrap();
        assert!(c.warnings().is_empty());
    }
}
