use crate::{Checked, Options, Problem, Severity, Version, versions};

/// Lays `layers` over `base`, key by key, into [`Checked`] options for
/// `version`.
///
/// One set of options serves several versions, so an option this version
/// lacks, or a value of a kind it does not take, is left out and returned as a
/// [`Severity::Note`]. One that NO supported version would take is a mistake,
/// a typo or a pre-1.13 keybind, and fails the merge. Any `version:` in the
/// inputs is replaced by `version`'s.
pub fn merge(
    base: &Options,
    layers: &[Options],
    version: &'static Version,
) -> Result<(Checked, Vec<Problem>), Vec<Problem>> {
    let mut merged = base.clone();
    for layer in layers {
        layer.iter().for_each(|(k, v)| merged.set(k, v));
    }
    merged.remove("version");

    let fits =
        |v: &Version, key: &str, value: &str| v.kind(key).and_then(|k| k.parse(value)).is_some();
    let mut out = Options::default();
    let (mut notes, mut problems) = (Vec::new(), Vec::new());
    for (key, value) in merged.iter() {
        if fits(version, key, value) {
            out.set(key, value);
        } else if versions().any(|v| v.defaults.get(key).is_some() && fits(v, key, value)) {
            let message = format!(
                "{key}:{value} left out: Minecraft {} does not take it",
                version.name
            );
            notes.push(Problem::new(Severity::Note, None, message));
        } else {
            let message = format!("{key}:{value} is not an option any supported Minecraft takes");
            problems.push(Problem::new(Severity::Error, None, message));
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok((Checked::new(&out, version)?, notes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn latest() -> &'static Version {
        versions().last().unwrap()
    }

    #[test]
    fn stamps_the_version_and_later_layers_win() {
        let v = latest();
        let base = Options::parse("fov:0.0\nmaxFps:60\n");
        let layers = [
            Options::parse("version:1\nfov:1.0\nkey_key.mymod.open:key.keyboard.b\n"),
            Options::parse("maxFps:120\n"),
        ];
        let (out, notes) = merge(&base, &layers, v).unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(
            out.to_string(),
            format!(
                "version:{}\nfov:1.0\nmaxFps:120\nkey_key.mymod.open:key.keyboard.b\n",
                v.data_version
            )
        );
    }

    #[test]
    fn fails_on_what_no_version_takes() {
        let v = latest();
        for bad in ["renderDistanse:12\n", "key_key.use:57\n", "bobView:yes\n"] {
            let p = merge(&Options::default(), &[Options::parse(bad)], v).unwrap_err();
            assert_eq!(p.len(), 1, "{bad}");
            assert_eq!(p[0].severity, Severity::Error);
        }
    }

    #[test]
    fn leaves_out_an_option_only_other_versions_have() {
        let (old, new) = (versions().next().unwrap(), latest());
        let (key, value) = new
            .defaults
            .iter()
            .find(|(k, _)| old.defaults.get(k).is_none())
            .expect("the newest version has an option the oldest lacks");
        let layer = Options::parse(&format!("{key}:{value}"));
        let (out, notes) = merge(&Options::default(), std::slice::from_ref(&layer), old).unwrap();
        assert!(out.get(key).is_none());
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].severity, Severity::Note);
        let (out, notes) = merge(&Options::default(), &[layer], new).unwrap();
        assert!(out.get(key).is_some() && notes.is_empty());
    }

    #[test]
    fn carries_the_checks_warnings() {
        let v = latest();
        let layer = Options::parse("graphicsPreset:\"fancy\"\nsimulationDistance:6\n");
        let (out, _) = merge(&Options::default(), &[layer], v).unwrap();
        assert_eq!(out.warnings().len(), 1);
    }
}
