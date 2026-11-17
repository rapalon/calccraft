# Plan: Universal 2 macOS Binary in CI

## Overview
This document specifies the transition of macOS release artifacts from two separate CPU-architecture archives (`calculator-<version>-macos-arm64.zip` and `calculator-<version>-macos-x64.zip`) to a single **Universal 2** application bundle (`calculator-<version>-macos-universal.zip`), as outlined in item 3 (P1) of `docs/next-steps.md`.

## Motivation
- **User experience:** Mac users currently have to identify whether their Mac uses Apple Silicon (M1/M2/M3/M4) or an Intel processor to pick the correct download.
- **macOS standard:** Universal 2 (fat Mach-O binary containing both `x86_64` and `arm64` slices) is the standard distribution format for macOS desktop applications.
- **Simplified release process:** Combining the two architectures reduces the total number of release assets from 5 to 4 and prevents confusion.

## Proposed Changes

### 1. GitHub Actions Workflow (`.github/workflows/release.yml`)

1. **Dedicated macOS job (`macos`):**
   - Runner: `macos-14` (Apple Silicon M1 runner, free for public repositories on GitHub Actions).
   - Rust targets installed: `aarch64-apple-darwin` and `x86_64-apple-darwin`.
   - Caching: `Swatinem/rust-cache@v2` keyed for macOS universal build.
   - Run tests: `cargo test --locked` (native Apple Silicon).
   - Compilation:
     ```bash
     cargo build --release --locked --target aarch64-apple-darwin
     cargo build --release --locked --target x86_64-apple-darwin
     ```

2. **Universal binary creation (`lipo`):**
   - Combine binaries:
     ```bash
     lipo -create -output "$app/Contents/MacOS/calculator" \
       target/aarch64-apple-darwin/release/calculator \
       target/x86_64-apple-darwin/release/calculator
     ```
   - Verification:
     Execute `lipo -info "$app/Contents/MacOS/calculator"` and check that both `arm64` and `x86_64` architectures are present in the output. Fail the job with a non-zero exit code if either is missing:
     ```bash
     info=$(lipo -info "$app/Contents/MacOS/calculator")
     echo "$info"
     if ! echo "$info" | grep -q "arm64" || ! echo "$info" | grep -q "x86_64"; then
       echo "::error::Universal binary must contain both arm64 and x86_64 architectures"
       exit 1
     fi
     ```

3. **Packaging:**
   - Preserve existing `Info.plist` creation.
   - Retain ad-hoc code signing (`codesign --force --deep --sign - "$app"`).
   - Package bundle into zip using `ditto -c -k --keepParent "$app" "dist/$asset.zip"`.
   - Asset naming: `calculator-$GITHUB_REF_NAME-macos-universal.zip`.
   - Compute SHA-256 checksum file: `calculator-$GITHUB_REF_NAME-macos-universal.zip.sha256`.
   - Upload asset and checksum to the draft release.

4. **Code Signing & Notarization Placeholder:**
   - Place clear comments and placeholder variables where Apple Developer ID code signing and Apple Notary Service (`xcrun notarytool`) commands will be executed once an Apple Developer account is configured.
   - Specify required GitHub repository secrets:
     - `APPLE_CERTIFICATE_BASE64`
     - `APPLE_CERTIFICATE_PASSWORD`
     - `APPLE_ID`
     - `APPLE_APP_SPECIFIC_PASSWORD`
     - `APPLE_TEAM_ID`

5. **Release Matrix & Dependencies:**
   - Update `build` job matrix in `release.yml` to build `windows-x64` and `linux-x64` only.
   - Update `publish` job dependencies to `needs: [build, macos, android]`.

### 2. Documentation Updates (`README.md`, `AGENTS.md`)
- Update the download table in `README.md`: replace separate Apple Silicon and Intel macOS rows with a single Universal 2 row (`calculator-<version>-macos-universal.zip`).
- Update the release section in `README.md` to reflect the 4 published platform packages and the new universal zip filename.
- Update reference in `AGENTS.md` describing release targets.
