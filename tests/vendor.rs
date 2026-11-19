//! The copy of accesskit_winit in `vendor/` (android/README.md, "Accessibility") must be the
//! one Cargo builds. When eframe moves to a newer accesskit_winit, Cargo only warns that the
//! `[patch.crates-io]` entry is unused and builds the release instead, and the Android app
//! would silently lose its accessibility tree: the copy has to be re-applied to the new
//! release first. These tests make that a failure.

const LOCK: &str = include_str!("../Cargo.lock");
const MANIFEST: &str = include_str!("../vendor/accesskit_winit/Cargo.toml");
const ANDROID: &str = include_str!("../vendor/accesskit_winit/src/platform_impl/android.rs");

/// The `[[package]]` entry of `name` in the lockfile.
fn locked_package(name: &str) -> &'static str {
    let header = format!("name = \"{name}\"\n");
    LOCK.split("[[package]]")
        .find(|entry| entry.trim_start().starts_with(&header))
        .unwrap_or_else(|| panic!("Cargo.lock has no package {name}"))
}

/// The value of the first `version = "…"` line in `text`.
fn version(text: &str) -> &str {
    text.lines()
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("a version line")
}

#[test]
fn vendored_accesskit_winit_matches_the_lockfile() {
    let locked = locked_package("accesskit_winit");
    // A crate taken from `[patch]` has no `source`; one from crates.io has
    // `source = "registry+…"`, and then the copy in vendor/ isn't what is built.
    assert!(
        !locked.contains("source = "),
        "Cargo builds accesskit_winit from crates.io, not from vendor/accesskit_winit: \
         re-apply the copy to the version egui-winit asks for\n{locked}"
    );
    assert_eq!(version(locked), version(MANIFEST));
}

#[test]
fn vendored_android_adapter_names_its_upstream_version() {
    let upstream = format!("accesskit_winit {}", version(MANIFEST));
    assert!(
        ANDROID.contains(&upstream),
        "android.rs's header must name the release it was changed from ({upstream})"
    );
    // The two changes the header promises.
    assert!(ANDROID.contains("android/R$id"), "hooks the content view");
    assert!(
        ANDROID.contains("const FIRST_SDK: i32 = 26;"),
        "skips Android before 8.0"
    );
}
