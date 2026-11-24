//! The Android app's "one activity, one process" rule (android/README.md), checked on every
//! system: Google Play rejected 1.4.5 because the activity could be restarted (it froze: an
//! ANR) or created twice (the app ended). docs/play-rejection-1.4.5.md has the repros.

use std::path::Path;

fn read(path: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|error| panic!("{path}: {error}"))
        // A Windows checkout may have CRLF line ends.
        .replace("\r\n", "\n")
}

/// The value of the activity's attribute `name` (`android:name="value"`).
fn attribute<'a>(manifest: &'a str, name: &str) -> Option<&'a str> {
    let activity = &manifest[manifest.find("<activity")?..];
    let activity = &activity[..activity.find('>')?];
    let start = activity.find(&format!("{name}=\""))? + name.len() + 2;
    let rest = &activity[start..];
    Some(&rest[..rest.find('"')?])
}

/// The value of the `<meta-data>` named `name`.
fn meta_data<'a>(manifest: &'a str, name: &str) -> Option<&'a str> {
    let at = manifest.find(&format!("android:name=\"{name}\""))?;
    let rest = &manifest[at..];
    let start = rest.find("android:value=\"")? + "android:value=\"".len();
    let rest = &rest[start..];
    Some(&rest[..rest.find('"')?])
}

/// Every configuration change an app can declare (`R.attr.configChanges`, API 36). The
/// undeclared `fontWeightAdjustment` (Bold text) restarted the activity, which froze the app.
const DECLARABLE: [&str; 18] = [
    "mcc",
    "mnc",
    "locale",
    "touchscreen",
    "keyboard",
    "keyboardHidden",
    "navigation",
    "orientation",
    "screenLayout",
    "uiMode",
    "screenSize",
    "smallestScreenSize",
    "layoutDirection",
    "fontScale",
    "density",
    "colorMode",
    "grammaticalGender",
    "fontWeightAdjustment",
];

#[test]
fn every_configuration_change_is_handled_by_the_app() {
    let manifest = read("android/AndroidManifest.xml");
    let declared: Vec<&str> = attribute(&manifest, "android:configChanges")
        .expect("the activity has configChanges")
        .split('|')
        .collect();
    for change in DECLARABLE {
        assert!(
            declared.contains(&change),
            "configChanges lacks {change}: that change would restart the activity"
        );
    }
}

#[test]
fn an_intent_for_the_running_activity_goes_to_it() {
    // `standard` stacked a second NativeActivity (`am start -n` while the app ran), whose
    // `android_main` found winit's event loop taken: the app ended.
    let manifest = read("android/AndroidManifest.xml");
    assert_eq!(
        attribute(&manifest, "android:launchMode"),
        Some("singleTask")
    );
}

#[test]
fn native_activity_starts_through_the_destroy_hook() {
    // NativeActivity calls the function `android.app.func_name` names; it must be the one in
    // src/lib.rs that wraps onDestroy, or a destroyed activity freezes the app again.
    let manifest = read("android/AndroidManifest.xml");
    assert_eq!(
        meta_data(&manifest, "android.app.func_name"),
        Some("calccraft_on_create")
    );
    assert_eq!(
        meta_data(&manifest, "android.app.lib_name"),
        Some("calculator")
    );
    let lib = read("src/lib.rs");
    let entry = lib
        .find("unsafe extern \"C\" fn calccraft_on_create(")
        .expect("src/lib.rs defines calccraft_on_create");
    assert!(
        lib[..entry].trim_end().ends_with("#[unsafe(no_mangle)]"),
        "calccraft_on_create must be exported unmangled"
    );
    let body = &lib[entry..];
    let body = &body[..body.find("\n}\n").expect("the function ends")];
    assert!(body.contains("ANativeActivity_onCreate(activity, saved_state, saved_state_size)"));
    assert!(body.contains("callbacks.onDestroy = Some(end_on_destroy)"));
    assert!(lib.contains("unsafe { libc::_exit(0) }"));
}
