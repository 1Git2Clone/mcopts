//! End to end: the mcopts binary on real files in a scratch directory.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use mcopts::versions;

/// A fresh scratch directory per test.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mcopts-e2e-{}-{test}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn put(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn mcopts(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mcopts"))
        .args(args)
        .output()
        .unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn newest() -> &'static mcopts::Version {
    versions().last().unwrap()
}

fn oldest() -> &'static mcopts::Version {
    versions().next().unwrap()
}

#[test]
fn versions_lists_every_supported_version() {
    let o = mcopts(&["versions"]);
    assert!(o.status.success());
    let lines: Vec<String> = stdout(&o).lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), versions().count());
    let v = newest();
    assert_eq!(
        lines.last().unwrap(),
        &format!("{}\t{}\t{}", v.name, v.data_version, v.defaults.len())
    );
}

#[test]
fn check_passes_good_warns_and_fails_bad() {
    let dir = scratch("check");
    let v = newest();
    let file = dir.join("options.txt");
    let f = file.to_str().unwrap();

    put(&file, &format!("version:{}\nfov:1.0\n", v.data_version));
    let o = mcopts(&["check", "-m", v.name, f]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(stderr(&o), "");

    put(
        &file,
        &format!(
            "version:{}\ngraphicsPreset:\"fancy\"\nsimulationDistance:6\n",
            v.data_version
        ),
    );
    let o = mcopts(&["check", "-m", v.name, f]);
    assert!(o.status.success());
    assert!(
        stderr(&o).contains("warning: graphicsPreset"),
        "{}",
        stderr(&o)
    );

    put(&file, "fov:1.0\nbobView:yes\n");
    let o = mcopts(&["check", "-m", v.name, f]);
    assert!(!o.status.success());
    let e = stderr(&o);
    assert!(e.contains("error: line 2: bobView"), "{e}");
    assert!(e.contains("error: no version: line"), "{e}");
}

#[test]
fn check_refuses_an_unknown_version_and_a_missing_file() {
    let o = mcopts(&["check", "-m", "1.20.1", "options.txt"]);
    assert!(!o.status.success());
    assert!(
        stderr(&o).contains("1.20.1 is not supported"),
        "{}",
        stderr(&o)
    );

    let o = mcopts(&["check", "-m", newest().name, "/nonexistent/options.txt"]);
    assert!(!o.status.success());
    assert!(
        stderr(&o).starts_with("mcopts: /nonexistent/options.txt: "),
        "{}",
        stderr(&o)
    );
}

#[test]
fn apply_layers_dirs_stamps_the_version_and_copies_configs() {
    let dir = scratch("apply");
    let (all, mine, out) = (dir.join("all"), dir.join("mine"), dir.join("out"));
    put(&all.join("options.txt"), "# shared\nfov:0.5\nmaxFps:60\n");
    put(&all.join("config/sodium-options.json"), "{\"a\":1}");
    put(&mine.join("options.txt"), "fov:1.0\n");
    put(&mine.join("config/sodium-options.json"), "{\"a\":2}");
    put(&out.join("options.txt"), "guiScale:3\nfov:0.0\n");
    let v = newest();

    let o = mcopts(&[
        "apply",
        "-m",
        v.name,
        out.to_str().unwrap(),
        all.to_str().unwrap(),
        mine.to_str().unwrap(),
    ]);
    assert!(o.status.success(), "{}", stderr(&o));
    let written = fs::read_to_string(out.join("options.txt")).unwrap();
    assert_eq!(
        written,
        format!(
            "version:{}\nguiScale:3\nfov:1.0\nmaxFps:60\n",
            v.data_version
        )
    );
    assert_eq!(
        fs::read_to_string(out.join("config/sodium-options.json")).unwrap(),
        "{\"a\":2}"
    );

    // What apply writes, check passes.
    let o = mcopts(&[
        "check",
        "-m",
        v.name,
        out.join("options.txt").to_str().unwrap(),
    ]);
    assert!(o.status.success(), "{}", stderr(&o));
}

#[test]
fn apply_notes_options_an_older_version_lacks() {
    let dir = scratch("apply-old");
    let (old, new) = (oldest(), newest());
    let (key, value) = new
        .defaults
        .iter()
        .find(|(k, _)| old.defaults.get(k).is_none())
        .unwrap();
    let (src, out) = (dir.join("src"), dir.join("out"));
    put(
        &src.join("options.txt"),
        &format!("fov:1.0\n{key}:{value}\n"),
    );

    let o = mcopts(&[
        "apply",
        "-m",
        old.name,
        out.to_str().unwrap(),
        src.to_str().unwrap(),
    ]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(
        stderr(&o).contains(&format!("note: {key}:{value} left out")),
        "{}",
        stderr(&o)
    );
    let written = fs::read_to_string(out.join("options.txt")).unwrap();
    assert!(!written.contains(key), "{written}");
}

#[test]
fn apply_fails_on_a_typo_and_leaves_the_target_alone() {
    let dir = scratch("apply-typo");
    let (src, out) = (dir.join("src"), dir.join("out"));
    put(&src.join("options.txt"), "renderDistanse:12\n");
    put(&out.join("options.txt"), "fov:0.0\n");

    let o = mcopts(&[
        "apply",
        "-m",
        newest().name,
        out.to_str().unwrap(),
        src.to_str().unwrap(),
    ]);
    assert!(!o.status.success());
    assert!(
        stderr(&o).contains("renderDistanse:12 is not an option"),
        "{}",
        stderr(&o)
    );
    assert_eq!(
        fs::read_to_string(out.join("options.txt")).unwrap(),
        "fov:0.0\n"
    );
}

#[test]
fn sync_takes_preferences_and_keeps_the_header() {
    let dir = scratch("sync");
    let (instance, target) = (dir.join("instance"), dir.join("target"));
    put(
        &instance.join("options.txt"),
        "version:5023\nfov:1.0\nguiScale:2\nlastServer:example.org\nresourcePacks:[\"vanilla\"]\n",
    );
    put(&instance.join("config/sodium-options.json"), "{}");
    put(&instance.join("config/lithium.properties"), "x=1");
    put(
        &target.join("options.txt"),
        "# mine\n\nfov:0.5\nmaxFps:60\n",
    );

    let o = mcopts(&[
        "sync",
        instance.to_str().unwrap(),
        target.to_str().unwrap(),
        "--except",
        "guiScale",
    ]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(
        stdout(&o),
        "1 options, 1 config files\n  config/sodium-options.json\n"
    );
    assert_eq!(
        fs::read_to_string(target.join("options.txt")).unwrap(),
        "# mine\n\nfov:1.0\nmaxFps:60\n"
    );
    assert!(target.join("config/sodium-options.json").exists());
    assert!(!target.join("config/lithium.properties").exists());
}

#[test]
fn diff_lists_changes_and_fails_on_any() {
    let dir = scratch("diff");
    let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
    put(&a, "fov:1\nguiScale:2\n");
    put(&b, "fov:1.0\nguiScale:0\n");

    let o = mcopts(&["diff", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(!o.status.success());
    assert_eq!(stdout(&o), "guiScale: 2 -> 0\n");

    let o = mcopts(&["diff", b.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(o.status.success());
    assert_eq!(stdout(&o), "");
}
