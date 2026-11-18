//! The vibration lab (Cargo feature `vibration-lab`, test builds only; docs/vibration-lab.md):
//! a page with one button per way Android can make a key tap, to compare them by feel on each
//! phone, and a dump of what the phone's vibrator reports, also written to logcat with the tag
//! `CalcCraftVibLab`.
//!
//! Its texts are English literals, not `locales/` strings: it is a measuring tool for the team
//! and never ships (the test at the bottom keeps the feature out of every release build).

use std::sync::OnceLock;

use eframe::egui::{
    self, Align2, FontId, Id, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, pos2, vec2,
};

use super::icons::ICON_BACK;
use super::{CalculatorApp, HEADER_HEIGHT, Kind, Page, RADIUS, access, icon_label, semibold};

/// What an entry plays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Effect {
    /// `VibrationEffect.Composition.addPrimitive(id, scale)`, available from `min_sdk`.
    Primitive { id: i32, scale: f32, min_sdk: i32 },
    /// `VibrationEffect.createPredefined(id)` (Android 10+).
    Predefined { id: i32 },
    /// `View.performHapticFeedback(constant)` on the activity's decor view.
    Haptic { constant: i32, min_sdk: i32 },
    /// `VibrationEffect.createOneShot(ms, amplitude)`, on a vibrator with amplitude control.
    Amplitude { amplitude: i32, ms: i64 },
    /// `VibrationEffect.createOneShot(ms, DEFAULT_AMPLITUDE)`.
    Duration { ms: i64 },
}

/// One button of the lab: a short id to report by, a plain label and what it plays.
pub(super) struct Entry {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) effect: Effect,
}

// `VibrationEffect.Composition`'s primitives.
const PRIMITIVE_CLICK: i32 = 1;
const PRIMITIVE_THUD: i32 = 2;
const PRIMITIVE_QUICK_RISE: i32 = 4;
const PRIMITIVE_TICK: i32 = 7;
const PRIMITIVE_LOW_TICK: i32 = 8;
/// Every primitive with its name, for the capabilities (SPIN, SLOW_RISE and QUICK_FALL have
/// no button, but the dump lists them).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const PRIMITIVES: [(i32, &str); 8] = [
    (PRIMITIVE_CLICK, "CLICK"),
    (PRIMITIVE_THUD, "THUD"),
    (3, "SPIN"),
    (PRIMITIVE_QUICK_RISE, "QUICK_RISE"),
    (5, "SLOW_RISE"),
    (6, "QUICK_FALL"),
    (PRIMITIVE_TICK, "TICK"),
    (PRIMITIVE_LOW_TICK, "LOW_TICK"),
];

// `VibrationEffect`'s predefined effects.
const EFFECT_CLICK: i32 = 0;
const EFFECT_DOUBLE_CLICK: i32 = 1;
const EFFECT_TICK: i32 = 2;
const EFFECT_HEAVY_CLICK: i32 = 5;
/// The effects `areEffectsSupported` is asked about, with their names.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const EFFECTS: [(i32, &str); 4] = [
    (EFFECT_CLICK, "CLICK"),
    (EFFECT_TICK, "TICK"),
    (EFFECT_HEAVY_CLICK, "HEAVY_CLICK"),
    (EFFECT_DOUBLE_CLICK, "DOUBLE_CLICK"),
];
/// `Vibrator.VIBRATION_EFFECT_SUPPORT_NO`; 0 is UNKNOWN and 1 YES.
const EFFECT_SUPPORT_NO: i32 = 2;

// `HapticFeedbackConstants`. `KEYBOARD_PRESS` is `KEYBOARD_TAP`'s other name (both 3).
const VIRTUAL_KEY: i32 = 1;
const KEYBOARD_TAP: i32 = 3;
const KEYBOARD_PRESS: i32 = 3;
const CONFIRM: i32 = 16;

const fn primitive(id: &'static str, label: &'static str, primitive: i32, scale: f32) -> Entry {
    let min_sdk = if primitive == PRIMITIVE_LOW_TICK {
        31
    } else {
        30
    };
    Entry {
        id,
        label,
        effect: Effect::Primitive {
            id: primitive,
            scale,
            min_sdk,
        },
    }
}

const fn predefined(id: &'static str, label: &'static str, effect: i32) -> Entry {
    Entry {
        id,
        label,
        effect: Effect::Predefined { id: effect },
    }
}

const fn haptic(id: &'static str, label: &'static str, constant: i32, min_sdk: i32) -> Entry {
    Entry {
        id,
        label,
        effect: Effect::Haptic { constant, min_sdk },
    }
}

const fn amplitude(id: &'static str, label: &'static str, amplitude: i32, ms: i64) -> Entry {
    Entry {
        id,
        label,
        effect: Effect::Amplitude { amplitude, ms },
    }
}

const fn duration(id: &'static str, label: &'static str, ms: i64) -> Entry {
    Entry {
        id,
        label,
        effect: Effect::Duration { ms },
    }
}

/// Every entry, in the page's order: the same list on every phone.
#[rustfmt::skip]
pub(super) const ENTRIES: [Entry; 25] = [
    primitive("P1", "Click primitive, scale 1.0", PRIMITIVE_CLICK, 1.0),
    primitive("P2", "Click primitive, scale 0.5", PRIMITIVE_CLICK, 0.5),
    primitive("P3", "Tick primitive, scale 1.0", PRIMITIVE_TICK, 1.0),
    primitive("P4", "Low tick primitive, scale 1.0", PRIMITIVE_LOW_TICK, 1.0),
    primitive("P5", "Thud primitive, scale 1.0", PRIMITIVE_THUD, 1.0),
    primitive("P6", "Quick rise primitive, scale 1.0", PRIMITIVE_QUICK_RISE, 1.0),
    predefined("E1", "Predefined click", EFFECT_CLICK),
    predefined("E2", "Predefined heavy click", EFFECT_HEAVY_CLICK),
    predefined("E3", "Predefined tick", EFFECT_TICK),
    predefined("E4", "Predefined double click", EFFECT_DOUBLE_CLICK),
    haptic("H1", "View haptic KEYBOARD_TAP (old key tap)", KEYBOARD_TAP, 1),
    haptic("H2", "View haptic VIRTUAL_KEY", VIRTUAL_KEY, 1),
    haptic("H3", "View haptic CONFIRM", CONFIRM, 30),
    haptic("H4", "View haptic KEYBOARD_PRESS (= KEYBOARD_TAP)", KEYBOARD_PRESS, 27),
    amplitude("A1", "Amplitude 255, 5 ms", 255, 5),
    amplitude("A2", "Amplitude 255, 8 ms", 255, 8),
    amplitude("A3", "Amplitude 255, 10 ms", 255, 10),
    amplitude("A4", "Amplitude 255, 15 ms", 255, 15),
    amplitude("A5", "Amplitude 255, 20 ms", 255, 20),
    amplitude("A6", "Amplitude 255, 30 ms", 255, 30),
    amplitude("A7", "Amplitude 128, 15 ms", 128, 15),
    amplitude("A8", "Amplitude 60, 15 ms", 60, 15),
    duration("D1", "Default amplitude, 10 ms", 10),
    duration("D2", "Default amplitude, 20 ms", 20),
    duration("D3", "Default amplitude, 40 ms", 40),
];

/// What the phone can play, as far as the entries need it.
#[derive(Clone, Debug, Default)]
pub(super) struct Support {
    /// `Build.VERSION.SDK_INT`, 0 off Android.
    pub(super) sdk: i32,
    pub(super) has_vibrator: bool,
    pub(super) amplitude_control: bool,
    /// The supported primitives (`areAllPrimitivesSupported`, one at a time).
    pub(super) primitives: Vec<i32>,
    /// `areEffectsSupported`'s answers, by effect.
    pub(super) effects: Vec<(i32, i32)>,
}

/// Whether an entry can play here.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Availability {
    Ready,
    /// It plays, with something worth knowing about it.
    Note(String),
    /// It can't play, for this reason (the button is greyed).
    Unavailable(String),
}

/// Whether `effect` can play with `support`.
pub(super) fn availability(effect: Effect, support: &Support) -> Availability {
    use Availability::*;
    if support.sdk == 0 {
        return Unavailable("not Android: no vibrator".into());
    }
    let needs = |min_sdk: i32| {
        (support.sdk < min_sdk)
            .then(|| Unavailable(format!("needs API {min_sdk} (this is {})", support.sdk)))
    };
    if let Effect::Haptic { min_sdk, .. } = effect {
        return needs(min_sdk).unwrap_or(Ready);
    }
    if !support.has_vibrator {
        return Unavailable("no vibrator".into());
    }
    match effect {
        Effect::Primitive { id, min_sdk, .. } => needs(min_sdk).unwrap_or_else(|| {
            if support.primitives.contains(&id) {
                Ready
            } else {
                Unavailable("primitive not supported".into())
            }
        }),
        Effect::Predefined { id } => needs(29).unwrap_or_else(|| {
            match support.effects.iter().find(|(effect, _)| *effect == id) {
                Some((_, EFFECT_SUPPORT_NO)) => {
                    Note("effect reported unsupported: plays Android's fallback".into())
                }
                Some((_, 0)) => Note("effect support unknown".into()),
                _ => Ready,
            }
        }),
        Effect::Amplitude { .. } => needs(26).unwrap_or_else(|| {
            if support.amplitude_control {
                Ready
            } else {
                Unavailable("no amplitude control".into())
            }
        }),
        Effect::Duration { .. } => Ready,
        Effect::Haptic { .. } => unreachable!(),
    }
}

/// The capabilities: what the entries need, and the dump's lines.
struct Capabilities {
    support: Support,
    lines: Vec<String>,
}

/// The capabilities, asked once: they don't change while the app runs.
fn capabilities() -> &'static Capabilities {
    static CAPABILITIES: OnceLock<Capabilities> = OnceLock::new();
    CAPABILITIES.get_or_init(|| {
        #[cfg(target_os = "android")]
        if let Some((support, lines)) = jni_lab::probe() {
            return Capabilities { support, lines };
        }
        Capabilities {
            support: Support::default(),
            lines: vec![if cfg!(target_os = "android") {
                "The vibrator couldn't be asked (JNI failed, see the CalcCraft log)".into()
            } else {
                "Not Android: no vibrator".into()
            }],
        }
    })
}

/// Called when the page opens: writes the capabilities to the log, every time, so a fresh
/// `adb logcat -s CalcCraftVibLab` after opening the page has them.
pub(super) fn opened() {
    let lines = &capabilities().lines;
    #[cfg(target_os = "android")]
    jni_lab::log_lines(lines);
    // Desktops have no vibrator, and no log to write it to.
    #[cfg(not(target_os = "android"))]
    let _ = lines;
}

/// Plays `effect` once. Returns what happened, for the line under the header.
fn play(effect: Effect) -> Result<(), String> {
    #[cfg(target_os = "android")]
    return jni_lab::play(effect);
    #[cfg(not(target_os = "android"))]
    {
        let _ = effect;
        Err("not Android".into())
    }
}

impl CalculatorApp {
    /// Draws the lab page in `rect`: a header with the back button, the entries' buttons and
    /// the capabilities.
    pub(super) fn draw_vibration_lab(&mut self, ui: &mut Ui, rect: Rect) {
        let p = self.palette;
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), HEADER_HEIGHT));
        let back = Rect::from_center_size(
            pos2(header.left() + 24.0, header.center().y),
            vec2(40.0, 36.0),
        );
        if self.button(
            ui,
            back,
            Id::new("vibration-lab-back"),
            Kind::Subtle,
            true,
            icon_label(ICON_BACK, 16.0),
            super::tr("TitleBarBackButton.AutomationName"),
        ) {
            self.page = Page::Settings;
        }
        ui.painter().text(
            pos2(header.left() + 48.0, header.center().y),
            Align2::LEFT_CENTER,
            "Vibration lab",
            FontId::new(20.0, semibold()),
            p.text,
        );

        let caps = capabilities();
        let last_id = Id::new("vibration-lab-last");
        let last: String = ui
            .ctx()
            .data(|d| d.get_temp(last_id))
            .unwrap_or_else(|| "Tap a button: it plays once".to_owned());
        let mut played = None;
        ui.scope_builder(
            UiBuilder::new().max_rect(rect.with_min_y(header.bottom())),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("vibration-lab")
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        const SIDE: f32 = 16.0;
                        const ROW: f32 = 60.0;
                        let width = ui.available_width();
                        let painter = ui.painter().clone();
                        let dump = painter.layout(
                            caps.lines.join("\n"),
                            FontId::monospace(11.0),
                            p.text,
                            width - 2.0 * SIDE - 24.0,
                        );
                        let status_y = 20.0;
                        let entries_top = 44.0;
                        let caps_y = entries_top + ENTRIES.len() as f32 * (ROW + 6.0) + 16.0;
                        let height = caps_y + 28.0 + dump.size().y + 24.0 + SIDE;
                        let (area, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
                        let (left, right, top) =
                            (area.left() + SIDE, area.right() - SIDE, area.top());
                        painter.text(
                            pos2(left, top + status_y),
                            Align2::LEFT_CENTER,
                            &last,
                            FontId::proportional(14.0),
                            p.text_secondary,
                        );
                        for (i, entry) in ENTRIES.iter().enumerate() {
                            let y = top + entries_top + i as f32 * (ROW + 6.0);
                            let row = Rect::from_x_y_ranges(left..=right, y..=y + ROW);
                            let availability = availability(entry.effect, &caps.support);
                            let enabled = !matches!(availability, Availability::Unavailable(_));
                            let name = format!("{} {}", entry.id, entry.label);
                            let response = access::control(
                                ui,
                                p,
                                row,
                                Id::new("vibration-lab").with(entry.id),
                                enabled,
                                RADIUS,
                                access::button(enabled, &name),
                            );
                            let fill = if !enabled {
                                p.digit_disabled
                            } else if response.is_pointer_button_down_on() {
                                p.digit_pressed
                            } else if response.hovered() {
                                p.digit_hover
                            } else {
                                p.digit
                            };
                            painter.rect_filled(row, RADIUS, fill);
                            painter.rect_stroke(
                                row,
                                RADIUS,
                                Stroke::new(1.0, p.card_border),
                                StrokeKind::Inside,
                            );
                            let (text, secondary) = if enabled {
                                (p.text, p.text_secondary)
                            } else {
                                (p.text_disabled, p.text_disabled)
                            };
                            let note = match &availability {
                                Availability::Ready => None,
                                Availability::Note(note) | Availability::Unavailable(note) => {
                                    Some(note.as_str())
                                }
                            };
                            let label_y = if note.is_some() {
                                row.top() + 21.0
                            } else {
                                row.center().y
                            };
                            painter.text(
                                pos2(row.left() + 16.0, label_y),
                                Align2::LEFT_CENTER,
                                entry.id,
                                FontId::new(20.0, semibold()),
                                text,
                            );
                            painter.text(
                                pos2(row.left() + 60.0, label_y),
                                Align2::LEFT_CENTER,
                                entry.label,
                                FontId::proportional(15.0),
                                text,
                            );
                            if let Some(note) = note {
                                painter.text(
                                    pos2(row.left() + 60.0, row.top() + 43.0),
                                    Align2::LEFT_CENTER,
                                    note,
                                    FontId::proportional(12.0),
                                    secondary,
                                );
                            }
                            if response.clicked() {
                                played = Some(entry);
                            }
                        }
                        painter.text(
                            pos2(left, top + caps_y + 10.0),
                            Align2::LEFT_CENTER,
                            "Capabilities",
                            FontId::new(16.0, semibold()),
                            p.text,
                        );
                        let card = Rect::from_x_y_ranges(
                            left..=right,
                            top + caps_y + 28.0..=top + caps_y + 28.0 + dump.size().y + 24.0,
                        );
                        painter.rect_filled(card, RADIUS, p.card);
                        painter.rect_stroke(
                            card,
                            RADIUS,
                            Stroke::new(1.0, p.card_border),
                            StrokeKind::Inside,
                        );
                        painter.galley(card.min + vec2(12.0, 12.0), dump, p.text);
                    });
            },
        );
        if let Some(entry) = played {
            let outcome = match play(entry.effect) {
                Ok(()) => format!("Played {}: {}", entry.id, entry.label),
                Err(error) => format!("{} failed: {error}", entry.id),
            };
            ui.ctx().data_mut(|d| d.insert_temp(last_id, outcome));
        }
    }
}

/// The lab's JNI calls: the probe, the log and the effects.
#[cfg(target_os = "android")]
mod jni_lab {
    use jni::JNIEnv;
    use jni::objects::{JIntArray, JObject, JString, JThrowable, JValue, JValueGen, JValueOwned};

    use super::super::android::{decor_view, one_shot, sdk_version, vibrator, with_activity};
    use super::{EFFECTS, Effect, PRIMITIVE_LOW_TICK, PRIMITIVES, Support};

    /// The tag of the lab's log lines.
    const TAG: &str = "CalcCraftVibLab";
    /// `VibrationEffect.DEFAULT_AMPLITUDE`.
    const DEFAULT_AMPLITUDE: i32 = -1;

    /// `result`, with a Java exception (a method this Android lacks, a refused hidden API, a
    /// `SecurityException`) cleared and turned into its text, so the next call works.
    fn attempt<T>(env: &mut JNIEnv, result: jni::errors::Result<T>) -> Result<T, String> {
        result.map_err(|error| {
            if env.exception_check().unwrap_or(false) {
                let throwable: JThrowable = env.exception_occurred().unwrap_or_default();
                env.exception_clear().ok();
                if let Some(text) = to_string(env, &throwable) {
                    return text;
                }
            }
            error.to_string()
        })
    }

    /// A number a getter returned, as text.
    fn number(value: &JValueOwned) -> String {
        match value {
            JValueGen::Int(int) => int.to_string(),
            JValueGen::Long(long) => long.to_string(),
            JValueGen::Float(float) => float.to_string(),
            JValueGen::Bool(bool) => (*bool != 0).to_string(),
            _ => "?".into(),
        }
    }

    /// `object.toString()`, or `None` if that fails too.
    fn to_string(env: &mut JNIEnv, object: &JObject) -> Option<String> {
        if object.is_null() {
            return Some("null".into());
        }
        let text = env
            .call_method(object, "toString", "()Ljava/lang/String;", &[])
            .and_then(|value| value.l());
        match text {
            Ok(text) => env.get_string(&JString::from(text)).ok().map(Into::into),
            Err(_) => {
                env.exception_clear().ok();
                None
            }
        }
    }

    fn string_field(env: &mut JNIEnv, class: &str, name: &str) -> String {
        let value = env
            .get_static_field(class, name, "Ljava/lang/String;")
            .and_then(|value| value.l());
        match attempt(env, value) {
            Ok(value) => to_string(env, &value).unwrap_or_default(),
            Err(error) => error,
        }
    }

    fn int_array<'local>(
        env: &mut JNIEnv<'local>,
        values: &[i32],
    ) -> jni::errors::Result<JIntArray<'local>> {
        let array = env.new_int_array(values.len() as i32)?;
        env.set_int_array_region(&array, 0, values)?;
        Ok(array)
    }

    fn read_ints(env: &mut JNIEnv, array: JObject) -> jni::errors::Result<Vec<i32>> {
        let array = JIntArray::from(array);
        let mut values = vec![0; env.get_array_length(&array)? as usize];
        env.get_int_array_region(&array, 0, &mut values)?;
        Ok(values)
    }

    /// Asks the vibrator everything docs/vibration-lab.md lists. `None` when JNI fails as a
    /// whole (no activity).
    pub(super) fn probe() -> Option<(Support, Vec<String>)> {
        with_activity(|env, activity| {
            let mut lines = Vec::new();
            let mut support = Support::default();
            let sdk = sdk_version(env)?;
            support.sdk = sdk;
            let release = string_field(env, "android/os/Build$VERSION", "RELEASE");
            lines.push(format!("SDK {sdk} (Android {release})"));
            let manufacturer = string_field(env, "android/os/Build", "MANUFACTURER");
            let model = string_field(env, "android/os/Build", "MODEL");
            let device = string_field(env, "android/os/Build", "DEVICE");
            lines.push(format!("Device: {manufacturer} {model} ({device})"));

            let found = vibrator(env, activity, sdk);
            let vibrator = match attempt(env, found) {
                Ok(vibrator) => vibrator,
                Err(error) => {
                    lines.push(format!("Vibrator service: {error}"));
                    None
                }
            };
            support.has_vibrator = vibrator.is_some();
            lines.push(format!("hasVibrator: {}", support.has_vibrator));
            if let Some(vibrator) = &vibrator {
                probe_vibrator(env, activity, vibrator, &mut support, &mut lines);
            }
            settings(env, activity, &mut lines);
            Ok((support, lines))
        })
    }

    fn probe_vibrator(
        env: &mut JNIEnv,
        activity: &JObject,
        vibrator: &JObject,
        support: &mut Support,
        lines: &mut Vec<String>,
    ) {
        let sdk = support.sdk;
        if sdk >= 26 {
            let result = env
                .call_method(vibrator, "hasAmplitudeControl", "()Z", &[])
                .and_then(|value| value.z());
            match attempt(env, result) {
                Ok(control) => support.amplitude_control = control,
                Err(error) => lines.push(format!("hasAmplitudeControl failed: {error}")),
            }
        }
        lines.push(format!(
            "hasAmplitudeControl: {}",
            support.amplitude_control
        ));

        if sdk >= 31 {
            let result = (|| {
                let name = env.new_string("vibrator_manager")?;
                let manager = env
                    .call_method(
                        activity,
                        "getSystemService",
                        "(Ljava/lang/String;)Ljava/lang/Object;",
                        &[JValue::Object(&name)],
                    )?
                    .l()?;
                let ids = env
                    .call_method(&manager, "getVibratorIds", "()[I", &[])?
                    .l()?;
                read_ints(env, ids)
            })();
            let line = match attempt(env, result) {
                Ok(ids) => format!("VibratorManager ids: {ids:?}"),
                Err(error) => format!("VibratorManager ids: {error}"),
            };
            lines.push(line);
            for (method, signature) in [
                ("getId", "()I"),
                ("getResonantFrequency", "()F"),
                ("getQFactor", "()F"),
            ] {
                let result = env.call_method(vibrator, method, signature, &[]);
                let line = match attempt(env, result) {
                    Ok(value) => format!("{method}: {}", number(&value)),
                    Err(error) => format!("{method}: {error}"),
                };
                lines.push(line);
            }
        } else {
            lines.push(format!(
                "getId, getResonantFrequency, getQFactor: need API 31 (this is {sdk})"
            ));
        }

        // The hidden `getInfo()`: its `toString` has the capabilities, when Android lets an
        // app call it.
        let result = env
            .call_method(vibrator, "getInfo", "()Landroid/os/VibratorInfo;", &[])
            .and_then(|value| value.l());
        let line = match attempt(env, result) {
            Ok(info) => format!(
                "VibratorInfo (hidden getInfo): {}",
                to_string(env, &info).unwrap_or_default()
            ),
            Err(error) => format!("VibratorInfo (hidden getInfo): not readable: {error}"),
        };
        lines.push(line);

        // The primitives, one by one, and their durations.
        if sdk >= 30 {
            let mut supported = Vec::new();
            for (id, name) in PRIMITIVES {
                if id == PRIMITIVE_LOW_TICK && sdk < 31 {
                    lines.push(format!("Primitive {name}: needs API 31"));
                    continue;
                }
                let result = (|| {
                    let ids = int_array(env, &[id])?;
                    let all = env
                        .call_method(
                            vibrator,
                            "areAllPrimitivesSupported",
                            "([I)Z",
                            &[JValue::Object(&ids)],
                        )?
                        .z()?;
                    let duration = if sdk >= 31 {
                        let durations = env
                            .call_method(
                                vibrator,
                                "getPrimitiveDurations",
                                "([I)[I",
                                &[JValue::Object(&ids)],
                            )?
                            .l()?;
                        read_ints(env, durations)?.first().copied()
                    } else {
                        None
                    };
                    Ok((all, duration))
                })();
                let line = match attempt(env, result) {
                    Ok((all, duration)) => {
                        if all {
                            supported.push(id);
                        }
                        let duration = duration.map_or(String::new(), |ms| format!(", {ms} ms"));
                        format!(
                            "Primitive {name}: {}{duration}",
                            if all { "supported" } else { "NOT supported" }
                        )
                    }
                    Err(error) => format!("Primitive {name}: {error}"),
                };
                lines.push(line);
            }
            support.primitives = supported;

            let ids: Vec<i32> = EFFECTS.iter().map(|(id, _)| *id).collect();
            let result = (|| {
                let array = int_array(env, &ids)?;
                let answers = env
                    .call_method(
                        vibrator,
                        "areEffectsSupported",
                        "([I)[I",
                        &[JValue::Object(&array)],
                    )?
                    .l()?;
                read_ints(env, answers)
            })();
            match attempt(env, result) {
                Ok(answers) => {
                    let words: Vec<String> = EFFECTS
                        .iter()
                        .zip(&answers)
                        .map(|((_, name), answer)| {
                            let word = match answer {
                                1 => "yes",
                                2 => "NO",
                                _ => "unknown",
                            };
                            format!("{name} {word}")
                        })
                        .collect();
                    lines.push(format!("Effects supported: {}", words.join(", ")));
                    support.effects = ids.into_iter().zip(answers).collect();
                }
                Err(error) => lines.push(format!("areEffectsSupported: {error}")),
            }
        } else {
            lines.push(format!(
                "Primitives and effect support: need API 30 (this is {sdk})"
            ));
        }

        if sdk >= 36 {
            api_36(env, vibrator, lines);
        } else {
            lines.push(format!(
                "Envelope effects and frequency profile: need API 36 (this is {sdk})"
            ));
        }
    }

    /// Android 16's envelope effects and frequency profile.
    fn api_36(env: &mut JNIEnv, vibrator: &JObject, lines: &mut Vec<String>) {
        let result = env
            .call_method(vibrator, "areEnvelopeEffectsSupported", "()Z", &[])
            .and_then(|value| value.z());
        let line = match attempt(env, result) {
            Ok(supported) => format!("areEnvelopeEffectsSupported: {supported}"),
            Err(error) => format!("areEnvelopeEffectsSupported: missing: {error}"),
        };
        lines.push(line);
        let getters = [
            (
                "getEnvelopeEffectInfo",
                "()Landroid/os/vibrator/EnvelopeEffectInfo;",
                &[
                    ("getMaxSize", "()I"),
                    ("getMaxDurationMillis", "()J"),
                    ("getMinControlPointDurationMillis", "()J"),
                    ("getMaxControlPointDurationMillis", "()J"),
                ][..],
            ),
            (
                "getFrequencyProfile",
                "()Landroid/os/vibrator/VibratorFrequencyProfile;",
                &[
                    ("getMinFrequencyHz", "()F"),
                    ("getMaxFrequencyHz", "()F"),
                    ("getMaxOutputAccelerationGs", "()F"),
                ][..],
            ),
        ];
        for (getter, signature, fields) in getters {
            let result = env
                .call_method(vibrator, getter, signature, &[])
                .and_then(|value| value.l());
            let object = match attempt(env, result) {
                Ok(object) if object.is_null() => {
                    lines.push(format!("{getter}: null"));
                    continue;
                }
                Ok(object) => object,
                Err(error) => {
                    lines.push(format!("{getter}: missing: {error}"));
                    continue;
                }
            };
            let mut values = Vec::new();
            for (field, signature) in fields {
                let result = env.call_method(&object, field, signature, &[]);
                let value = match attempt(env, result) {
                    Ok(value) => number(&value),
                    Err(error) => format!("missing ({error})"),
                };
                values.push(format!("{field} {value}"));
            }
            lines.push(format!("{getter}: {}", values.join(", ")));
        }
    }

    /// The touch feedback settings, each with its value or why it can't be read.
    fn settings(env: &mut JNIEnv, activity: &JObject, lines: &mut Vec<String>) {
        const NAMES: [&str; 6] = [
            "haptic_feedback_enabled",
            "haptic_feedback_intensity",
            "hardware_haptic_feedback_intensity",
            "keyboard_vibration_enabled",
            // Samsung's own.
            "VIB_FEEDBACK_MAGNITUDE",
            "SEM_VIBRATION_FORCE_TOUCH_INTENSITY",
        ];
        let resolver = env
            .call_method(
                activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )
            .and_then(|value| value.l());
        let resolver = match attempt(env, resolver) {
            Ok(resolver) => resolver,
            Err(error) => {
                lines.push(format!("Settings: {error}"));
                return;
            }
        };
        for name in NAMES {
            let result = (|| {
                let key = env.new_string(name)?;
                env.call_static_method(
                    "android/provider/Settings$System",
                    "getString",
                    "(Landroid/content/ContentResolver;Ljava/lang/String;)Ljava/lang/String;",
                    &[JValue::Object(&resolver), JValue::Object(&key)],
                )?
                .l()
            })();
            let line = match attempt(env, result) {
                Ok(value) if value.is_null() => format!("Settings.System {name}: unset"),
                Ok(value) => format!(
                    "Settings.System {name}: {}",
                    to_string(env, &value).unwrap_or_default()
                ),
                Err(error) => format!("Settings.System {name}: not readable: {error}"),
            };
            lines.push(line);
        }
    }

    /// Writes `lines` to logcat with the lab's tag (`android.util.Log.i`).
    pub(super) fn log_lines(lines: &[String]) {
        with_activity(|env, _| {
            let tag = env.new_string(TAG)?;
            for line in lines {
                let text = env.new_string(line)?;
                env.call_static_method(
                    "android/util/Log",
                    "i",
                    "(Ljava/lang/String;Ljava/lang/String;)I",
                    &[JValue::Object(&tag), JValue::Object(&text)],
                )?;
                env.delete_local_ref(text)?;
            }
            Ok(())
        });
    }

    /// Plays `effect` once.
    pub(super) fn play(effect: Effect) -> Result<(), String> {
        with_activity(|env, activity| {
            let result = play_in(env, activity, effect);
            Ok(attempt(env, result).and_then(|played| played))
        })
        .unwrap_or_else(|| Err("JNI failed".into()))
    }

    fn play_in(
        env: &mut JNIEnv,
        activity: &JObject,
        effect: Effect,
    ) -> jni::errors::Result<Result<(), String>> {
        let sdk = sdk_version(env)?;
        if let Effect::Haptic { constant, .. } = effect {
            let decor = decor_view(env, activity)?;
            let done = env
                .call_method(
                    &decor,
                    "performHapticFeedback",
                    "(I)Z",
                    &[JValue::Int(constant)],
                )?
                .z()?;
            return Ok(if done {
                Ok(())
            } else {
                Err("performHapticFeedback returned false".into())
            });
        }
        let Some(vibrator) = vibrator(env, activity, sdk)? else {
            return Ok(Err("no vibrator".into()));
        };
        let effect = match effect {
            Effect::Primitive { id, scale, .. } => {
                let composition = env
                    .call_static_method(
                        "android/os/VibrationEffect",
                        "startComposition",
                        "()Landroid/os/VibrationEffect$Composition;",
                        &[],
                    )?
                    .l()?;
                let composition = env
                    .call_method(
                        &composition,
                        "addPrimitive",
                        "(IF)Landroid/os/VibrationEffect$Composition;",
                        &[JValue::Int(id), JValue::Float(scale)],
                    )?
                    .l()?;
                env.call_method(
                    &composition,
                    "compose",
                    "()Landroid/os/VibrationEffect;",
                    &[],
                )?
                .l()?
            }
            Effect::Predefined { id } => env
                .call_static_method(
                    "android/os/VibrationEffect",
                    "createPredefined",
                    "(I)Landroid/os/VibrationEffect;",
                    &[JValue::Int(id)],
                )?
                .l()?,
            Effect::Amplitude { amplitude, ms } => one_shot(env, ms, amplitude)?,
            Effect::Duration { ms } => {
                if sdk < 26 {
                    env.call_method(&vibrator, "vibrate", "(J)V", &[JValue::Long(ms)])?;
                    return Ok(Ok(()));
                }
                one_shot(env, ms, DEFAULT_AMPLITUDE)?
            }
            Effect::Haptic { .. } => unreachable!(),
        };
        env.call_method(
            &vibrator,
            "vibrate",
            "(Landroid/os/VibrationEffect;)V",
            &[JValue::Object(&effect)],
        )?;
        Ok(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phone(sdk: i32) -> Support {
        Support {
            sdk,
            has_vibrator: true,
            amplitude_control: true,
            primitives: vec![PRIMITIVE_CLICK, PRIMITIVE_TICK, PRIMITIVE_LOW_TICK],
            effects: vec![(EFFECT_CLICK, 1), (EFFECT_HEAVY_CLICK, EFFECT_SUPPORT_NO)],
        }
    }

    fn entry(id: &str) -> Effect {
        ENTRIES.iter().find(|entry| entry.id == id).unwrap().effect
    }

    #[test]
    fn entries_are_the_specified_ones() {
        let ids: Vec<&str> = ENTRIES.iter().map(|entry| entry.id).collect();
        assert_eq!(
            ids,
            [
                "P1", "P2", "P3", "P4", "P5", "P6", "E1", "E2", "E3", "E4", "H1", "H2", "H3", "H4",
                "A1", "A2", "A3", "A4", "A5", "A6", "A7", "A8", "D1", "D2", "D3"
            ]
        );
        assert_eq!(
            entry("P2"),
            Effect::Primitive {
                id: PRIMITIVE_CLICK,
                scale: 0.5,
                min_sdk: 30
            }
        );
        assert_eq!(
            entry("P4"),
            Effect::Primitive {
                id: PRIMITIVE_LOW_TICK,
                scale: 1.0,
                min_sdk: 31
            }
        );
        let one_shots: Vec<(i32, i64)> = ENTRIES
            .iter()
            .filter_map(|entry| match entry.effect {
                Effect::Amplitude { amplitude, ms } => Some((amplitude, ms)),
                _ => None,
            })
            .collect();
        assert_eq!(
            one_shots,
            [
                (255, 5),
                (255, 8),
                (255, 10),
                (255, 15),
                (255, 20),
                (255, 30),
                (128, 15),
                (60, 15)
            ]
        );
        assert_eq!(entry("D3"), Effect::Duration { ms: 40 });
        assert_eq!(
            entry("H1"),
            Effect::Haptic {
                constant: 3,
                min_sdk: 1
            }
        );
    }

    #[test]
    fn unsupported_entries_say_why() {
        let support = phone(36);
        assert_eq!(availability(entry("P1"), &support), Availability::Ready);
        assert_eq!(
            availability(entry("P5"), &support),
            Availability::Unavailable("primitive not supported".into())
        );
        // An effect reported unsupported still plays Android's fallback.
        assert!(matches!(
            availability(entry("E2"), &support),
            Availability::Note(_)
        ));
        assert_eq!(availability(entry("E1"), &support), Availability::Ready);

        let old = Support {
            amplitude_control: false,
            ..phone(30)
        };
        assert_eq!(
            availability(entry("P4"), &old),
            Availability::Unavailable("needs API 31 (this is 30)".into())
        );
        assert_eq!(
            availability(entry("A1"), &old),
            Availability::Unavailable("no amplitude control".into())
        );
        assert_eq!(availability(entry("D1"), &old), Availability::Ready);
        assert_eq!(
            availability(entry("H3"), &phone(29)),
            Availability::Unavailable("needs API 30 (this is 29)".into())
        );

        // Off Android everything is greyed, and still listed.
        let none = Support::default();
        assert!(ENTRIES.iter().all(|entry| matches!(
            availability(entry.effect, &none),
            Availability::Unavailable(_)
        )));
        // Without a vibrator, the view's haptics are still worth a try.
        let mute = Support {
            has_vibrator: false,
            ..phone(36)
        };
        assert_eq!(availability(entry("H1"), &mute), Availability::Ready);
        assert_eq!(
            availability(entry("D1"), &mute),
            Availability::Unavailable("no vibrator".into())
        );
    }
}
