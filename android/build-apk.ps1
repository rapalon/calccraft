<#
.SYNOPSIS
Builds the Android app as an APK for 64-bit ARM phones and tablets.

.DESCRIPTION
Builds the Rust library for Android with cargo-ndk, then packages it with the Android SDK's
own tools, without Gradle: javac and d8 (the share provider in java/), aapt2 (manifest and
resources), zipalign (16 KB pages for the
uncompressed library, as Google Play requires) and apksigner. Runs on Windows, Linux (as in
GitHub Actions) and macOS. See android/README.md.

The application ID defaults to com.rapalon.calccraft (or APPLICATION_ID in the environment)
and can be overridden with -ApplicationId.

-Channel picks the distribution channel (docs/support-prompt.md, section 8; or
CALCCRAFT_CHANNEL in the environment):
- default: GitHub releases and F-Droid. The support popup's Donate opens the donate page.
- play-free: the free Google Play app (Cargo feature play-free). No donate link: the popup's
  button opens the paid app's Play listing, -PaidApplicationId (or CALCCRAFT_PAID_ID). Without
  it the support prompt is hidden, which is how the first free release ships.
- play-paid: the paid Google Play app (Cargo feature play-paid), with -PaidApplicationId as its
  application ID. No support prompt.
The files are target/android/CalcCraft-<version>-arm64.apk/.aab, CalcCraft-Play-... for
play-free and CalcCraft-Pro-... for play-paid.
A Play build fails if its library contains the donate page's or Stripe's address.

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
pwsh android/build-apk.ps1 -Bundle

.EXAMPLE
pwsh android/build-apk.ps1 -Channel play-free -Bundle
# The free Google Play app; add -PaidApplicationId com.rapalon.calccraft.pro once the paid app
# is published, so its popup leads there.

.EXAMPLE
pwsh android/build-apk.ps1 -Channel play-paid -PaidApplicationId com.rapalon.calccraft.pro -Bundle
#>
param(
    [string]$ApplicationId = $(if ($env:APPLICATION_ID) { $env:APPLICATION_ID } else { 'com.rapalon.calccraft' }),
    [ValidateSet('default', 'play-free', 'play-paid')]
    [string]$Channel = $(if ($env:CALCCRAFT_CHANNEL) { $env:CALCCRAFT_CHANNEL } else { 'default' }),
    # The paid Google Play app's application ID (com.rapalon.calccraft.pro), unset by default.
    [string]$PaidApplicationId = $(if ($env:CALCCRAFT_PAID_ID) { $env:CALCCRAFT_PAID_ID } else { '' }),
    # Also build the App Bundle, target/android/CalcCraft-<version>-arm64.aab.
    [switch]$Bundle,
    # With -Bundle, also the universal APK bundletool makes from the bundle (build-apks
    # --mode=universal), target/android/CalcCraft-<version>-arm64-universal.apk, signed with the
    # same key: the code Play installs, for trying a release before uploading it.
    [switch]$UniversalApk,
    # Mark the app debuggable (android:debuggable), so `adb shell run-as` can look into its
    # files, as when checking that the graph share keeps one file in cache/share/. Test builds only.
    [switch]$Debuggable,
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
if ($UniversalApk -and -not $Bundle) {
    throw '-UniversalApk needs -Bundle: the universal APK is made from the App Bundle.'
}

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
$javac = 'javac'
if ($Jdk) {
    $env:JAVA_HOME = $Jdk
    $javac = Join-Path $Jdk "bin/javac$exe"
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

# The channel: its Cargo feature, the paid app's ID compiled in (CALCCRAFT_PAID_ID, read with
# option_env!), and the files' name.
$packageName = '^[A-Za-z][A-Za-z0-9_]*(\.[A-Za-z][A-Za-z0-9_]*)+$'
if ($PaidApplicationId -and $PaidApplicationId -notmatch $packageName) {
    throw "-PaidApplicationId '$PaidApplicationId' is not an Android package name."
}
$features = @()
$name = 'CalcCraft'
Remove-Item Env:CALCCRAFT_PAID_ID -ErrorAction SilentlyContinue
switch ($Channel) {
    'play-free' {
        $features = @('--features', 'play-free')
        $name = 'CalcCraft-Play'
        if ($PaidApplicationId) {
            $env:CALCCRAFT_PAID_ID = $PaidApplicationId
            "Google Play, free: the support popup leads to $PaidApplicationId"
        } else {
            'Google Play, free: no paid app given, so the support prompt is hidden'
        }
    }
    'play-paid' {
        if (-not $PaidApplicationId) {
            throw '-Channel play-paid needs -PaidApplicationId (or CALCCRAFT_PAID_ID): it is the application ID.'
        }
        $features = @('--features', 'play-paid')
        $ApplicationId = $PaidApplicationId
        $name = 'CalcCraft-Pro'
        "Google Play, paid: $ApplicationId, no support prompt"
    }
}

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
    cargo ndk -t arm64-v8a -P $minSdk rustc --lib --release @features --crate-type cdylib
} finally {
    Pop-Location
}
$strip = Join-Path $env:ANDROID_NDK_HOME "toolchains/llvm/prebuilt/$ndkHost/bin/llvm-strip$exe"
$library = Join-Path $libDir 'libcalculator.so'
& $strip --strip-unneeded -o $library (Join-Path $repo 'target/aarch64-linux-android/release/libcalculator.so')

# Google Play's Payments policy: a Play build must not lead to another payment method, so its
# library must not even contain the donate page's or Stripe's address.
if ($Channel -ne 'default') {
    $bytes = [System.IO.File]::ReadAllBytes($library)
    $text = [System.Text.Encoding]::Latin1.GetString($bytes)
    foreach ($forbidden in @('rapalon.com/calccraft/donate', 'stripe.com')) {
        if ($text.Contains($forbidden, [StringComparison]::OrdinalIgnoreCase)) {
            throw "The $Channel library contains '$forbidden': Play builds must have no donate link."
        }
    }
    "Checked: no donate or Stripe address in the $Channel library"
    # The free app's popup, locked Pro themes and "Get CalcCraft Pro" lead to the paid app's
    # listing: its ID must be compiled in (1.4.3's free build had none, and hid the prompt).
    if ($Channel -eq 'play-free' -and $PaidApplicationId) {
        if (-not $text.Contains("details?id=")) {
            throw "The play-free library has no Play listing address."
        }
        if (-not $text.Contains($PaidApplicationId)) {
            throw "The play-free library doesn't contain the paid app's ID $PaidApplicationId."
        }
        "Checked: the play-free library leads to $PaidApplicationId"
    }
}

# The Java code (java/: the graph share's ContentProvider) as classes.dex: javac against the
# platform's android.jar, Java 8 bytecode, then d8 from the pinned build-tools. Both come with
# the JDK and the SDK the build already uses: no library, no download. The file lists are
# arrays (@(…)): splatting a lone string would pass it character by character.
$androidJar = Join-Path $Sdk "platforms/$Platform/android.jar"
$classes = Join-Path $out 'classes'
$dexDir = Join-Path $staging 'dex'
Remove-Item -Recurse -Force $classes -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $classes, $dexDir | Out-Null
$sources = @((Get-ChildItem (Join-Path $PSScriptRoot 'java') -Recurse -Filter *.java).FullName)
& $javac -source 8 -target 8 -bootclasspath $androidJar '-Xlint:-options' -encoding UTF-8 -d $classes @sources
$classFiles = @((Get-ChildItem $classes -Recurse -Filter *.class).FullName)
& (Join-Path $tools "d8$script") --release --min-api $minSdk --lib $androidJar --output $dexDir @classFiles
$dex = Join-Path $dexDir 'classes.dex'
if (-not (Test-Path $dex)) {
    throw "d8 wrote no $dex."
}

# Manifest and resources (the launcher icon and the themes).
$resources = Join-Path $out 'resources.zip'
& (Join-Path $tools "aapt2$exe") compile --dir (Join-Path $PSScriptRoot 'res') -o $resources
# CalcCraft Pro (play-paid) has its own launcher icon: android/res-pro/ (tools/app-icon.py --pro)
# overrides res/'s icon files and background colour (aapt2 link -R). The free app keeps res/'s.
$overlay = @()
if ($Channel -eq 'play-paid') {
    $proResources = Join-Path $out 'resources-pro.zip'
    & (Join-Path $tools "aapt2$exe") compile --dir (Join-Path $PSScriptRoot 'res-pro') -o $proResources
    $overlay = @('-R', $proResources)
    'Launcher icon: CalcCraft Pro (android/res-pro/)'
}

$manifestPath = Join-Path $staging 'AndroidManifest.xml'
$manifestContent = Get-Content (Join-Path $PSScriptRoot 'AndroidManifest.xml') -Raw
$manifestContent = $manifestContent -replace 'package="[^"]+"', "package=`"$ApplicationId`""
# The share provider's authority, as Gradle's manifest placeholder would fill it in.
$manifestContent = $manifestContent.Replace('${applicationId}', $ApplicationId)
[System.IO.File]::WriteAllText($manifestPath, $manifestContent, (New-Object System.Text.UTF8Encoding $false))

$link = @('-I', (Join-Path $Sdk "platforms/$Platform/android.jar"), '--manifest', $manifestPath,
    '--min-sdk-version', $minSdk, '--target-sdk-version', $targetSdk,
    '--version-code', $versionCode, '--version-name', $version, $resources) + $overlay
if ($Debuggable) {
    $link += '--debug-mode'
}
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
    [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $dex, 'classes.dex',
        [System.IO.Compression.CompressionLevel]::Optimal) | Out-Null
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
$apk = Join-Path $out "$name-$version-arm64.apk"
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
    Copy-Item $dexDir (Join-Path $module 'dex') -Recurse
    $moduleZip = Join-Path $out 'base.zip'
    [System.IO.Compression.ZipFile]::CreateFromDirectory($module, $moduleZip)

    # BundleConfig.json keeps the library uncompressed and 16 KB aligned in the APKs Play builds.
    $config = Join-Path $PSScriptRoot 'BundleConfig.json'
    $unsignedAab = Join-Path $out 'unsigned.aab'
    & $java -jar $Bundletool build-bundle "--modules=$moduleZip" "--output=$unsignedAab" "--config=$config"

    # Play checks a bundle's JAR signature (jarsigner), not an APK signature.
    $aab = Join-Path $out "$name-$version-arm64.aab"
    & $jarsigner @jarSigning -sigalg SHA256withRSA -digestalg SHA-256 -signedjar $aab $unsignedAab $alias
    & $jarsigner -verify $aab
    & $java -jar $Bundletool validate "--bundle=$aab"
    Remove-Item $linked, $module, $moduleZip, $unsignedAab -Recurse -Force
    "{0} ({1:N1} MB)" -f $aab, ((Get-Item $aab).Length / 1MB)

    if ($UniversalApk) {
        # bundletool takes passwords only on the command line or from a file, so they go to
        # files that only this build reads and that are deleted right after.
        $passwords = Join-Path $out 'passwords'
        New-Item -ItemType Directory -Force $passwords | Out-Null
        try {
            if ($env:ANDROID_KEYSTORE) {
                $storePass = Join-Path $passwords 'store'
                $keyPass = Join-Path $passwords 'key'
                $storeValue = $env:ANDROID_KEYSTORE_PASSWORD
                $keyValue = if ($env:ANDROID_KEY_PASSWORD) { $env:ANDROID_KEY_PASSWORD } else { $storeValue }
                Set-Content -NoNewline -Path $storePass -Value $storeValue
                Set-Content -NoNewline -Path $keyPass -Value $keyValue
                $apksSigning = @("--ks=$env:ANDROID_KEYSTORE", "--ks-key-alias=$alias",
                    "--ks-pass=file:$storePass", "--key-pass=file:$keyPass")
            } else {
                $apksSigning = @("--ks=$DebugKeystore", "--ks-key-alias=$alias",
                    '--ks-pass=pass:android', '--key-pass=pass:android')
            }
            $apks = Join-Path $out 'universal.apks'
            & $java -jar $Bundletool build-apks "--bundle=$aab" "--output=$apks" --overwrite `
                --mode=universal @apksSigning
        } finally {
            Remove-Item $passwords -Recurse -Force
        }
        $extracted = Join-Path $out 'universal'
        if (Test-Path $extracted) { Remove-Item $extracted -Recurse -Force }
        [System.IO.Compression.ZipFile]::ExtractToDirectory($apks, $extracted)
        $universal = Join-Path $out "$name-$version-arm64-universal.apk"
        Move-Item -Force (Join-Path $extracted 'universal.apk') $universal
        & (Join-Path $tools "apksigner$script") verify $universal
        Remove-Item $apks, $extracted -Recurse -Force
        "{0} ({1:N1} MB)" -f $universal, ((Get-Item $universal).Length / 1MB)
    }
}
Remove-Item $resources, $staging, $classes -Recurse -Force
if ($overlay) { Remove-Item $proResources -Force }
