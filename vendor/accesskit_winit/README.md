# AccessKit winit adapter

This is the winit adapter for [AccessKit](https://accesskit.dev/). It exposes an AccessKit accessibility tree through the platform-native accessibility API on any platform supported by AccessKit. On platforms not supported by AccessKit, this adapter does nothing, but still compiles.

## Compatibility with async runtimes

The following only applies on Linux/Unix:

While this crate's API is purely blocking, it internally spawns asynchronous tasks on an executor.

- If you use tokio, make sure to enable the `tokio` feature of this crate.
- If you use another async runtime or if you don't use one at all, the default feature will suit your needs.

## Android activity compatibility

The Android implementation of this adapter currently only works with [GameActivity](https://developer.android.com/games/agdk/game-activity), which is one of the two activity implementations that winit currently supports.

## CalcCraft's copy

This directory is a copy of the crates.io release 0.32.2 (Apache-2.0, `LICENSE-APACHE`), which
CalcCraft's root `Cargo.toml` puts in place of the release with `[patch.crates-io]`. One file
differs from the release: `src/platform_impl/android.rs`, whose Android adapter works with
`NativeActivity` (the activity CalcCraft's Android app uses) by hooking the activity's content
view, and does nothing on Android versions before 8.0, where `accesskit_android` can't load its
Java delegate. The file's header says so. Everything else is the release's code; the examples
and dev-dependencies aren't carried over. See `android/README.md`, "Accessibility".
