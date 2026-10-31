//! The supported Minecraft versions, embedded from data/defaults/<v>.txt.

use std::sync::LazyLock;

use crate::{Error, Options, value::Kind};

static DEFAULTS: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/defaults.rs"));

static VERSIONS: LazyLock<Vec<Version>> = LazyLock::new(|| {
    DEFAULTS
        .iter()
        .map(|(name, text)| Version::new(name, text))
        .collect()
});

/// A supported Minecraft version: its data version and its default options,
/// as the game itself writes them.
#[derive(Debug)]
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

    /// The kind of value `key` takes here: its default's, or a key name for a
    /// keybind this version lacks (a mod's). None: the game drops the key.
    pub(crate) fn kind(&self, key: &str) -> Option<Kind> {
        match self.defaults.get(key) {
            Some(default) => Some(Kind::of(key, default)),
            None => key.starts_with("key_").then_some(Kind::Key),
        }
    }
}

/// Every supported version, oldest first.
pub fn versions() -> impl Iterator<Item = &'static Version> {
    VERSIONS.iter()
}

pub fn version(name: &str) -> Result<&'static Version, Error> {
    versions()
        .find(|v| v.name == name)
        .ok_or_else(|| Error::UnknownVersion(name.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn versions_are_oldest_first_with_rising_data_versions() {
        let dvs: Vec<u32> = versions().map(|v| v.data_version).collect();
        assert!(dvs.windows(2).all(|w| w[0] < w[1]), "{dvs:?}");
        assert_eq!(versions().next().unwrap().name, "1.21");
    }

    #[test]
    fn unknown_version_lists_the_known_ones() {
        let e = version("1.20.1").unwrap_err().to_string();
        assert!(
            e.contains("1.20.1 is not supported") && e.contains("1.21.1"),
            "{e}"
        );
    }

    #[test]
    fn kinds_come_from_the_defaults_and_mod_keybinds_are_keys() {
        let v = versions().last().unwrap();
        assert_eq!(v.kind("fov"), Some(Kind::Number));
        assert_eq!(v.kind("key_key.mymod.open"), Some(Kind::Key));
        assert_eq!(v.kind("renderDistanse"), None);
    }
}
