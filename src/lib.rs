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
//!
//! [`Options`] is a file as read, any version's, mods' keys and all.
//! [`Checked`] is one the game will load: the only way to get one is to pass
//! the checks ([`Checked::parse`], [`Checked::new`], [`merge`]), and only a
//! [`Checked`] is ever written as an options.txt ([`apply`]).

mod check;
mod dir;
mod error;
mod merge;
mod options;
mod value;
mod version;

pub use check::{Checked, Problem, Severity};
pub use dir::{NOT_SYNCED, Synced, apply, is_preference, read, sync};
pub use error::Error;
pub use merge::merge;
pub use options::{Options, diff};
pub use value::{KeyName, Number, Value};
pub use version::{Version, version, versions};
