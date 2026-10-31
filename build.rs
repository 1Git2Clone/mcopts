//! Embeds data/defaults/<version>.txt, one per Minecraft version, oldest first.

use std::{env, fs, path::Path};

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/defaults");
    println!("cargo::rerun-if-changed={}", dir.display());

    let mut versions: Vec<String> = fs::read_dir(&dir)
        .expect("data/defaults")
        .map(|e| e.expect("data/defaults entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    versions.sort_by_key(|v| {
        v.split('.')
            .map(|n| n.parse::<u32>().expect("version numbers"))
            .collect::<Vec<_>>()
    });

    let entries: String = versions
        .iter()
        .map(|v| {
            format!(
                "    ({v:?}, include_str!({:?})),\n",
                dir.join(format!("{v}.txt"))
            )
        })
        .collect();
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("defaults.rs");
    fs::write(out, format!("&[\n{entries}]")).unwrap();
}
