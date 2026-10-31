use std::fmt;

/// `options.txt` as read: key/value pairs in file order, each key once, any
/// version's and any mod's. See [`crate::Checked`] for one the game will load.
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parse_skips_lines_without_a_colon() {
        assert_eq!(Options::parse("junk\n\nfov:1.0\n").len(), 1);
    }

    #[test]
    fn set_keeps_position_remove_takes_it_out() {
        let mut o = Options::parse("a:1\nb:2\n");
        o.set("a", "3");
        o.set("c", "4");
        assert_eq!(o.to_string(), "a:3\nb:2\nc:4\n");
        assert_eq!(o.remove("b"), Some("2".into()));
        assert_eq!(o.remove("b"), None);
        assert_eq!(o.to_string(), "a:3\nc:4\n");
        assert!(!o.is_empty());
        assert!(Options::default().is_empty());
    }

    #[test]
    fn diff_compares_numbers_by_value() {
        let a = Options::parse("fov:1\nguiScale:2\nlang:en_us\n");
        let b = Options::parse("fov:1.0\nguiScale:0\n");
        assert_eq!(
            diff(&a, &b),
            vec![("guiScale", "2", Some("0")), ("lang", "en_us", None)]
        );
        assert!(diff(&b, &b).is_empty());
    }
}
