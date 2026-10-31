//! What each option's value is, read off the kind of its default.

use std::fmt;

/// The kind of value an option takes, read off its default. Black-box: no
/// ranges or enum members, only what a default's shape tells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Bool,
    Number,
    List,
    Quoted,
    Key,
    Text,
}

impl Kind {
    pub(crate) fn of(key: &str, default: &str) -> Kind {
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

    /// `text` as a value of this kind, if it is one.
    pub(crate) fn parse(self, text: &str) -> Option<Value> {
        Some(match self {
            Kind::Bool => Value::Bool(text.parse().ok()?),
            Kind::Number => Value::Number(Number::parse(text)?),
            Kind::List => {
                (text.starts_with('[') && text.ends_with(']')).then_some(())?;
                Value::List(text.into())
            }
            Kind::Quoted => {
                (text.len() >= 2 && text.starts_with('"') && text.ends_with('"')).then_some(())?;
                Value::Quoted(text.into())
            }
            Kind::Key => Value::Key(KeyName::new(text)?),
            Kind::Text => Value::Text(text.into()),
        })
    }

    pub(crate) fn describe(self) -> &'static str {
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

/// A checked option value. Displays exactly as it was written.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Number(Number),
    /// `[...]`, as written.
    List(String),
    /// `"..."`, quotes included.
    Quoted(String),
    Key(KeyName),
    Text(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Bool(b) => write!(f, "{b}"),
            Value::Number(n) => write!(f, "{n}"),
            Value::List(s) | Value::Quoted(s) | Value::Text(s) => f.write_str(s),
            Value::Key(k) => write!(f, "{k}"),
        }
    }
}

/// A number that keeps the text it was written as: the game writes `1.0`, and
/// a round trip must not turn it into `1`.
#[derive(Clone, Debug, PartialEq)]
pub struct Number {
    value: f64,
    text: String,
}

impl Number {
    pub fn parse(text: &str) -> Option<Self> {
        let value = text.parse::<f64>().ok().filter(|v| v.is_finite())?;
        Some(Number {
            value,
            text: text.into(),
        })
    }

    pub fn value(&self) -> f64 {
        self.value
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// `key.keyboard.b`, `key.mouse.left`: what keybinds have been since 1.13. A
/// bare number (a pre-1.13 LWJGL2 keycode) is not one, and makes the 1.13
/// fixer throw and the game discard the whole file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyName(String);

impl KeyName {
    pub fn new(text: &str) -> Option<Self> {
        let ok = ["key.keyboard.", "key.mouse."].iter().any(|p| {
            text.strip_prefix(p).is_some_and(|rest| {
                !rest.is_empty()
                    && rest
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c))
            })
        });
        ok.then(|| KeyName(text.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for KeyName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_is_read_off_the_default() {
        assert_eq!(Kind::of("bobView", "true"), Kind::Bool);
        assert_eq!(Kind::of("fov", "0.0"), Kind::Number);
        assert_eq!(Kind::of("resourcePacks", "[]"), Kind::List);
        assert_eq!(Kind::of("graphicsPreset", "\"fancy\""), Kind::Quoted);
        assert_eq!(Kind::of("key_key.jump", "key.keyboard.space"), Kind::Key);
        assert_eq!(Kind::of("lang", "en_us"), Kind::Text);
    }

    #[test]
    fn each_kind_takes_only_its_values() {
        let takes = |k: Kind, s: &str| k.parse(s).is_some();
        assert!(takes(Kind::Bool, "false") && !takes(Kind::Bool, "yes"));
        assert!(takes(Kind::Number, "-0.5") && !takes(Kind::Number, "1,5"));
        assert!(!takes(Kind::Number, "NaN") && !takes(Kind::Number, "inf"));
        assert!(takes(Kind::List, "[\"a:b\"]") && !takes(Kind::List, "[a"));
        assert!(takes(Kind::Quoted, "\"\"") && !takes(Kind::Quoted, "\""));
        assert!(takes(Kind::Key, "key.mouse.left") && !takes(Kind::Key, "57"));
        assert!(takes(Kind::Text, "") && takes(Kind::Text, "anything"));
        for k in [
            Kind::Bool,
            Kind::Number,
            Kind::List,
            Kind::Quoted,
            Kind::Key,
            Kind::Text,
        ] {
            assert!(!k.describe().is_empty());
        }
    }

    #[test]
    fn values_display_as_written() {
        for (kind, text) in [
            (Kind::Bool, "true"),
            (Kind::Number, "1.0"),
            (Kind::Number, "1"),
            (Kind::List, "[\"vanilla\"]"),
            (Kind::Quoted, "\"custom\""),
            (Kind::Key, "key.keyboard.left.shift"),
            (Kind::Text, "en_us"),
        ] {
            assert_eq!(kind.parse(text).unwrap().to_string(), text);
        }
        assert_eq!(Number::parse("0.75").unwrap().value(), 0.75);
    }

    #[test]
    fn key_names() {
        for ok in [
            "key.keyboard.b",
            "key.mouse.4",
            "key.keyboard.keypad.0",
            "key.keyboard.left.control",
        ] {
            assert_eq!(KeyName::new(ok).unwrap().as_str(), ok);
        }
        for bad in [
            "57",
            "key.keyboard.",
            "key.keyboard.B",
            "key.joystick.a",
            "keyboard.b",
        ] {
            assert!(KeyName::new(bad).is_none(), "{bad}");
        }
    }
}
