# Plan: Package ID Unification and Android Application ID Build Parameter

## Overview
This document specifies the unification of application and bundle identifiers across CalcCraft platforms to `com.rapalon.calccraft`, and the introduction of a configurable Android `applicationId` build parameter.

## Motivation & Background
1. **Consistent Identity:** CalcCraft is released across Windows, macOS, Linux, Android, and Web. A consistent base identifier (`com.rapalon.calccraft`) aligns the macOS bundle identifier (`CFBundleIdentifier`) and Android application ID (`package` / `applicationId`).
2. **Google Play Store Policy:**
   - Once an app with a given package name is uploaded to Google Play, that package name is permanent and cannot be changed.
   - Google Play strictly forbids converting a free app to a paid app.
   - The base open-source releases distributed via GitHub and F-Droid use `com.rapalon.calccraft`.
   - To support publishing a paid variant to Google Play in the future without changing codebase files or diverging branches, the Android packaging pipeline must accept an arbitrary `applicationId` as a build parameter (e.g. `com.rapalon.calccraft.play`).

## Specification of Changes

### 1. Unified Base Identifier (`com.rapalon.calccraft`)
- **macOS Bundle (`.github/workflows/release.yml`):**
  - Update `CFBundleIdentifier` in `Info.plist` generated during the universal macOS build from `com.github.rapalon.calccraft` to `com.rapalon.calccraft`.
- **Android Manifest (`android/AndroidManifest.xml`):**
  - Update the base `package` attribute from `io.github.rapalon.calccraft` to `com.rapalon.calccraft`.
- **Documentation & Instructions (`android/README.md`):**
  - Update any `adb uninstall` commands and references from `io.github.rapalon.calccraft` to `com.rapalon.calccraft`.

### 2. Configurable Android `applicationId` Parameter

#### Build Script (`android/build-apk.ps1`)
- Add parameter:
  ```powershell
  [string]$ApplicationId = $(if ($env:APPLICATION_ID) { $env:APPLICATION_ID } else { 'com.rapalon.calccraft' })
  ```
- **Non-destructive Manifest Generation:**
  - The committed `android/AndroidManifest.xml` remains untouched.
  - During build execution, `build-apk.ps1` reads `android/AndroidManifest.xml`, substitutes the `package="<id>"` attribute with `$ApplicationId`, and outputs the resulting manifest to `target/android/staging/AndroidManifest.xml`.
  - `aapt2 link` consumes the staging manifest via `--manifest $manifestPath`.
- **Default Artifact Output:**
  - Default build continues to produce `CalcCraft-<version>-arm64.apk` in `target/android/`.

#### GitHub Actions Workflow (`.github/workflows/release.yml`)
- In the `android` job of `release.yml`, pass `APPLICATION_ID` (supporting repository variable `vars.ANDROID_APPLICATION_ID` with fallback to `com.rapalon.calccraft`).
- Ensure default release artifact behaviour remains identical to previous builds.

### 3. Documentation
- Document parameter usage in `android/README.md`:
  - Show how to build with a custom ID: `pwsh android/build-apk.ps1 -ApplicationId com.rapalon.calccraft.play` or via `$env:APPLICATION_ID`.
  - State the Google Play policy: package names are permanent once published and free apps cannot be changed to paid apps.
