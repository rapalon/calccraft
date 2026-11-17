<#
.SYNOPSIS
Builds the Android app as an APK for 64-bit ARM phones and tablets.

.DESCRIPTION
Builds the Rust library for Android with cargo-ndk, then packages it with the Android SDK's
own tools, without Gradle: aapt2 (manifest and resources), zipalign (16 KB pages for the
uncompressed library, as Google Play requires) and apksigner. Runs on Windows, Linux (as in
GitHub Actions) and macOS. See android/README.md.

The application ID defaults to com.rapalon.calccraft (or APPLICATION_ID in the environment)
and can be overridden with -ApplicationId (e.g. for a paid Google Play release).

The APK is signed with the release key when ANDROID_KEYSTORE names a keystore file, with its
password in ANDROID_KEYSTORE_PASSWORD, the key's alias in ANDROID_KEY_ALIAS and the key's
password in ANDROID_KEY_PASSWORD (the keystore's when empty). Otherwise it is signed with the
debug key (~/.android/debug.keystore, created when missing), which is fine for `adb install`
but can't update an APK signed with another key.

With -Bundle it also writes the Android App Bundle (.aab) Google Play takes, from the same
library and manifest, with bundletool (downloaded once into .tools/ and checked against its
SHA-256) and signed with jarsigner and the same key. Play only takes a bundle signed with the
upload key: one signed with the debug key is for trying `bundletool build-apks` on.

.EXAMPLE
pwsh android/build-apk.ps1
adb install -r target/android/CalcCraft-1.2.0-arm64.apk

.EXAMPLE
pwsh android/build-apk.ps1 -ApplicationId com.rapalon.calccraft.play

.EXAMPLE
pwsh android/build-apk.ps1 -Bundle
#>
param(
    [string]$ApplicationId = $(if ($env:APPLICATION_ID) { $env:APPLICATION_ID } else { 'com.rapalon.calccraft' }),
    # Also build the App Bundle, target/android/CalcCraft-<version>-arm64.aab.
    [switch]$Bundle,
    # A bundletool jar to use instead of the pinned release in .tools/.
    [string]$Bundletool = '',
    [string]$Sdk = $(if ($env:ANDROID_HOME) { $env:ANDROID_HOME }
        elseif ($IsWindows) { Join-Path $env:LOCALAPPDATA 'Android/Sdk' }
        elseif ($IsMacOS) { Join-Path $HOME 'Library/Android/sdk' }
        else { Join-Path $HOME 'Android/Sdk' }),
    [string]$Ndk = '29.0.14206865',
    [string]$BuildTools = '36.0.0',
    [string]$Platform = 'android-36',
    # A JDK for apksigner, keytool, jarsigner and bundletool; without one, they come from the PATH.
    [string]$Jdk = $(if ($env:JAVA_HOME) { $env:JAVA_HOME }
        elseif ($IsWindows) { Join-Path $env:LOCALAPPDATA 'Android/jdk-21' }
        else { '' }),
    [string]$DebugKeystore = (Join-Path $HOME '.android/debug.keystore')
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

# The SDK's and NDK's tools are named and placed differently on each system.
$exe = if ($IsWindows) { '.exe' } else { '' }
$script = if ($IsWindows) { '.bat' } else { '' }
$ndkHost = if ($IsWindows) { 'windows-x86_64' } elseif ($IsMacOS) { 'darwin-x86_64' } else { 'linux-x86_64' }

$repo = Split-Path $PSScriptRoot
$out = Join-Path $repo 'target/android'
$tools = Join-Path $Sdk "build-tools/$BuildTools"
$env:ANDROID_HOME = $Sdk
$env:ANDROID_NDK_HOME = Join-Path $Sdk "ndk/$Ndk"
$keytool = 'keytool'
$jarsigner = 'jarsigner'
$java = 'java'
if ($Jdk) {
    $env:JAVA_HOME = $Jdk
    $keytool = Join-Path $Jdk "bin/keytool$exe"
    $jarsigner = Join-Path $Jdk "bin/jarsigner$exe"
    $java = Join-Path $Jdk "bin/java$exe"
}
# Google's bundletool, a release from https://github.com/google/bundletool/releases, pinned
# with the SHA-256 the release lists.
$bundletoolVersion = '1.18.3'
$bundletoolSha256 = 'a099cfa1543f55593bc2ed16a70a7c67fe54b1747bb7301f37fdfd6d91028e29'
# Android 7.0, the oldest version Android's developer verification covers.
$minSdk = 24
$targetSdk = 36

# The version from Cargo.toml: 1.2.0 is version code 10200.
$version = (Select-String -Path (Join-Path $repo 'Cargo.toml') -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$parts = $version.Split('.') | ForEach-Object { [int]$_ }
$versionCode = $parts[0] * 10000 + $parts[1] * 100 + $parts[2]

Remove-Item $out -Recurse -Force -ErrorAction SilentlyContinue
$staging = Join-Path $out 'staging'
$libDir = Join-Path $staging 'lib/arm64-v8a'
New-Item -ItemType Directory -Force $libDir | Out-Null

# The Rust library: a cdylib only for Android (desktops and the web build the binary).
Push-Location $repo
try {
    cargo ndk -t arm64-v8a -P $minSdk rustc --lib --release --crate-type cdylib
} finally {
    Pop-Location
}
$strip = Join-Path $env:ANDROID_NDK_HOME "toolchains/llvm/prebuilt/$ndkHost/bin/llvm-strip$exe"
& $strip --strip-unneeded -o (Join-Path $libDir 'libcalculator.so') (Join-Path $repo 'target/aarch64-linux-android/release/libcalculator.so')

# Manifest and resources (the launcher icon and the themes).
$resources = Join-Path $out 'resources.zip'
& (Join-Path $tools "aapt2$exe") compile --dir (Join-Path $PSScriptRoot 'res') -o $resources

$manifestPath = Join-Path $staging 'AndroidManifest.xml'
$manifestContent = Get-Content (Join-Path $PSScriptRoot 'AndroidManifest.xml') -Raw
$manifestContent = $manifestContent -replace 'package="[^"]+"', "package=`"$ApplicationId`""
[System.IO.File]::WriteAllText($manifestPath, $manifestContent, (New-Object System.Text.UTF8Encoding $false))

$link = @('-I', (Join-Path $Sdk "platforms/$Platform/android.jar"), '--manifest', $manifestPath,
    '--min-sdk-version', $minSdk, '--target-sdk-version', $targetSdk,
    '--version-code', $versionCode, '--version-name', $version, $resources)
$unaligned = Join-Path $out 'unaligned.apk'
& (Join-Path $tools "aapt2$exe") link -o $unaligned @link

# The library goes in uncompressed (stored), so Android maps it straight from the APK
# (`extractNativeLibs="false"`): .NET stores an entry added without compression, while
# `aapt add` compresses it whatever `-0` says.
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::Open($unaligned, 'Update')
try {
    [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip,
        (Join-Path $libDir 'libcalculator.so'), 'lib/arm64-v8a/libcalculator.so',
        [System.IO.Compression.CompressionLevel]::NoCompression) | Out-Null
} finally {
    $zip.Dispose()
}

$aligned = Join-Path $out 'aligned.apk'
& (Join-Path $tools "zipalign$exe") -f -P 16 4 $unaligned $aligned

# apksigner and jarsigner read the passwords from the environment (`env:`), so they never
# appear in a command line.
if ($env:ANDROID_KEYSTORE) {
    $keyPassword = if ($env:ANDROID_KEY_PASSWORD) { 'ANDROID_KEY_PASSWORD' } else { 'ANDROID_KEYSTORE_PASSWORD' }
    $signing = @('--ks', $env:ANDROID_KEYSTORE, '--ks-key-alias', $env:ANDROID_KEY_ALIAS,
        '--ks-pass', 'env:ANDROID_KEYSTORE_PASSWORD', '--key-pass', "env:$keyPassword")
    $jarSigning = @('-keystore', $env:ANDROID_KEYSTORE, '-storepass:env', 'ANDROID_KEYSTORE_PASSWORD',
        '-keypass:env', $keyPassword)
    $alias = $env:ANDROID_KEY_ALIAS
    'Signing with the release key'
} else {
    if (-not (Test-Path $DebugKeystore)) {
        New-Item -ItemType Directory -Force (Split-Path $DebugKeystore) | Out-Null
        & $keytool -genkeypair -keystore $DebugKeystore -storepass android -keypass android `
            -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 `
            -dname 'CN=Android Debug,O=Android,C=US'
    }
    $signing = @('--ks', $DebugKeystore, '--ks-key-alias', 'androiddebugkey',
        '--ks-pass', 'pass:android', '--key-pass', 'pass:android')
    $jarSigning = @('-keystore', $DebugKeystore, '-storepass', 'android', '-keypass', 'android')
    $alias = 'androiddebugkey'
    'Signing with the debug key'
}
$apk = Join-Path $out "CalcCraft-$version-arm64.apk"
& (Join-Path $tools "apksigner$script") sign @signing --out $apk $aligned
& (Join-Path $tools "apksigner$script") verify $apk
Remove-Item $unaligned, $aligned -Force
"{0} ({1:N1} MB)" -f $apk, ((Get-Item $apk).Length / 1MB)

if ($Bundle) {
    if (-not $Bundletool) {
        $Bundletool = Join-Path $repo ".tools/bundletool-all-$bundletoolVersion.jar"
        if (-not (Test-Path $Bundletool)) {
            New-Item -ItemType Directory -Force (Split-Path $Bundletool) | Out-Null
            Invoke-WebRequest -OutFile $Bundletool `
                "https://github.com/google/bundletool/releases/download/$bundletoolVersion/bundletool-all-$bundletoolVersion.jar"
        }
        # Checked at every build, not only after the download: the jar runs with the signing key.
        $hash = (Get-FileHash $Bundletool -Algorithm SHA256).Hash
        if ($hash -ne $bundletoolSha256) {
            Remove-Item $Bundletool
            throw "$Bundletool has SHA-256 $hash, not the expected $bundletoolSha256; deleted it."
        }
    }

    # Google Play builds the APKs itself, from an App Bundle: the same manifest, resources and
    # library in bundletool's layout, with the manifest and the resources as protocol buffers.
    $linked = Join-Path $out 'base-linked.zip'
    & (Join-Path $tools "aapt2$exe") link --proto-format -o $linked @link
    $module = Join-Path $out 'base'
    [System.IO.Compression.ZipFile]::ExtractToDirectory($linked, $module)
    New-Item -ItemType Directory -Force (Join-Path $module 'manifest') | Out-Null
    Move-Item (Join-Path $module 'AndroidManifest.xml') (Join-Path $module 'manifest/AndroidManifest.xml')
    Copy-Item (Join-Path $staging 'lib') (Join-Path $module 'lib') -Recurse
    $moduleZip = Join-Path $out 'base.zip'
    [System.IO.Compression.ZipFile]::CreateFromDirectory($module, $moduleZip)

    # BundleConfig.json keeps the library uncompressed and 16 KB aligned in the APKs Play builds.
    $config = Join-Path $PSScriptRoot 'BundleConfig.json'
    $unsignedAab = Join-Path $out 'unsigned.aab'
    & $java -jar $Bundletool build-bundle "--modules=$moduleZip" "--output=$unsignedAab" "--config=$config"

    # Play checks a bundle's JAR signature (jarsigner), not an APK signature.
    $aab = Join-Path $out "CalcCraft-$version-arm64.aab"
    & $jarsigner @jarSigning -sigalg SHA256withRSA -digestalg SHA-256 -signedjar $aab $unsignedAab $alias
    & $jarsigner -verify $aab
    & $java -jar $Bundletool validate "--bundle=$aab"
    Remove-Item $linked, $module, $moduleZip, $unsignedAab -Recurse -Force
    "{0} ({1:N1} MB)" -f $aab, ((Get-Item $aab).Length / 1MB)
}
Remove-Item $resources, $staging -Recurse -Force
