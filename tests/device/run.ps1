<#
.SYNOPSIS
    Runs CalcCraft's device scenarios (tests/device/scenarios/*.json) on an Android phone through
    adb and uiautomator, and prints a pass/fail table.

.DESCRIPTION
    Every step finds a control by its accessible name in a `uiautomator dump` (content-desc, else
    text) and taps the center of its bounds with `input tap`, the way a finger would; nothing is
    found by coordinates. The rules are the same as those of the host test
    (`device_scenarios_pass_on_the_host` in src/app/a11y_tests.rs) and of an AI agent driving
    Marionet (tests/device/README.md).

    The phone must be unlocked with its screen on, CalcCraft installed in English (the names are
    English), and no other UiAutomation client running (uiautomator dump fails then).

.EXAMPLE
    pwsh -File tests/device/run.ps1 -Serial 4A281FDAQ000KX -Package com.rapalon.calccraft.a11ytest

.EXAMPLE
    pwsh -File tests/device/run.ps1 -Serial emulator-5554 -Scenario token-edit,basic-calculations -Out target/device-results.md

.EXAMPLE
    pwsh -File tests/device/run.ps1 -SelfTest
    No device: checks this script against two recorded dumps (fixtures/window-dump.xml in
    portrait, window-dump-landscape.xml with the phone sideways) and every scenario's steps.
    CI runs it.
#>
[CmdletBinding()]
param(
    # The device's serial, as `adb devices` lists it.
    [string] $Serial,
    # Checks the script without a device (see the examples).
    [switch] $SelfTest,
    # The application id of the build under test (the a11ytest build: com.rapalon.calccraft.a11ytest).
    [string] $Package = 'com.rapalon.calccraft',
    # Scenario names (file names without .json); all of them by default.
    [string[]] $Scenario,
    # The adb executable.
    [string] $Adb = 'adb',
    # How often a step looks for its node before it fails, and how long it waits in between.
    [int] $Retries = 6,
    [int] $DelayMs = 400,
    # A file to write the result table to, as Markdown (for the release's notes).
    [string] $Out
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$scenarioDir = Join-Path $PSScriptRoot 'scenarios'
$dumpOnDevice = '/sdcard/calccraft-window-dump.xml'
$dumpHere = Join-Path ([IO.Path]::GetTempPath()) "calccraft-window-dump-$PID.xml"
# The self-test reads this recorded dump instead of the phone's.
$recordedDump = $null
$knownSteps = 'launch', 'open', 'tap', 'taps', 'tap_if_present', 'expect', 'expect_prefix',
    'expect_contains', 'expect_absent', 'display', 'expression', 'edition', 'scroll_to',
    'expect_checked'
# The app's edition for `edition` steps: CalcCraft Pro (an application ID ending in .pro) has
# every colour theme unlocked; the other Play builds are the free app.
function Get-Edition([string] $Id) { if ($Id -clike '*.pro') { 'pro' } else { 'free' } }

# adb with the device's serial. A simple function, not an advanced one: its arguments go to adb
# as they are, `-W` and `-n` of `am start` included.
function Invoke-Adb {
    $output = & $Adb -s $Serial @args 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "adb $($args -join ' ') failed ($LASTEXITCODE): $($output -join ' ')"
    }
    $output
}

# A displayed number or expression without digit groups and decimal separators, so that the
# scenarios' en-US numbers match the phone's regional format (1,000 / 1'000 / 6,25).
function ConvertTo-Loose([string] $Text) {
    # Escapes, not the characters: PowerShell takes a typographic apostrophe for a quote.
    $Text -replace "[,.' `u{2019}`u{00a0}`u{202f}]", ''
    <#   #>
}

# The window hierarchy now, as XML, from a uiautomator dump (the recorded one in the self-test).
function Get-Dump {
    $text = $null
    if ($recordedDump) { $text = [IO.File]::ReadAllText($recordedDump, [Text.Encoding]::UTF8) }
    for ($i = 0; $i -lt 3 -and -not $text; $i++) {
        & $Adb -s $Serial shell uiautomator dump $dumpOnDevice 2>&1 | Out-Null
        & $Adb -s $Serial pull $dumpOnDevice $dumpHere 2>&1 | Out-Null
        if ($LASTEXITCODE -eq 0 -and (Test-Path $dumpHere)) {
            $text = [IO.File]::ReadAllText($dumpHere, [Text.Encoding]::UTF8)
            if ($text -notmatch '<hierarchy') { $text = $null }
        }
        if (-not $text) { Start-Sleep -Milliseconds $DelayMs }
    }
    if (-not $text) { throw 'uiautomator dump gave no window hierarchy' }
    [xml]$text
}

# A node's `bounds` as left, top, right, bottom; $null when it has none.
function Get-Bounds([Xml.XmlElement] $Node) {
    if ($Node.GetAttribute('bounds') -notmatch '\[(-?\d+),(-?\d+)\]\[(-?\d+),(-?\d+)\]') { return $null }
    [int]$Matches[1], [int]$Matches[2], [int]$Matches[3], [int]$Matches[4]
}

# The app's nodes on screen now: name, center, from a uiautomator dump.
function Get-Screen {
    $xml = Get-Dump
    $nodes = foreach ($node in $xml.SelectNodes('//node')) {
        if ($node.GetAttribute('package') -ne $Package) { continue }
        $name = $node.GetAttribute('content-desc')
        if (-not $name) { $name = $node.GetAttribute('text') }
        if (-not $name) { continue }
        $bounds = Get-Bounds $node
        if (-not $bounds) { continue }
        $left, $top, $right, $bottom = $bounds
        # Marionet drops empty nodes too; a tap needs a point inside.
        if ($right -le $left -or $bottom -le $top) { continue }
        [pscustomobject]@{
            Name    = $name
            X       = [int](($left + $right) / 2)
            Y       = [int](($top + $bottom) / 2)
            Checked = $node.GetAttribute('checked') -ceq 'true'
        }
    }
    # One by one into the pipeline; nothing at all for an empty screen.
    $nodes | Where-Object { $null -ne $_ }
}

# The app's window: Left, Top, Width, Height in screen pixels, in the current orientation. It's
# the app's outermost node in the dump (NativeActivity's FrameLayout spans the window), whose
# bounds follow the orientation; `wm size` is the physical panel, portrait on a phone, so a
# swipe computed from it with the phone held sideways landed off the screen. Without an app
# node (a screen the app doesn't cover) `wm size` is swapped by the dump's `rotation`.
function Get-Window {
    $xml = Get-Dump
    foreach ($node in $xml.SelectNodes('//node')) {
        if ($node.GetAttribute('package') -ne $Package) { continue }
        $bounds = Get-Bounds $node
        if (-not $bounds) { continue }
        $left, $top, $right, $bottom = $bounds
        if ($right -le $left -or $bottom -le $top) { continue }
        # Document order: the first node of the app is the outermost.
        return [pscustomobject]@{ Left = $left; Top = $top; Width = $right - $left; Height = $bottom - $top }
    }
    $size = (Invoke-Adb shell wm size) -join ' '
    $width, $height = ConvertTo-WindowSize $size $xml.hierarchy.GetAttribute('rotation')
    [pscustomobject]@{ Left = 0; Top = 0; Width = $width; Height = $height }
}

# The panel's `wm size` ("Physical size: 1080x2340") turned by the dump's `rotation`
# (Surface.ROTATION_*, 0 to 3; 1 and 3 are the panel on its side), as Width, Height.
function ConvertTo-WindowSize([string] $WmSize, [string] $Rotation) {
    if ($WmSize -notmatch '(\d+)x(\d+)\s*$') { throw "no screen size in `"$WmSize`"" }
    $width, $height = [int]$Matches[1], [int]$Matches[2]
    if ([int]$Rotation % 2 -eq 1) { $width, $height = $height, $width }
    $width, $height
}

# The scroll_to swipe in a window: a finger drawn up its middle, from 65 % of its height to 35 %,
# as X, FromY, ToY.
function Get-ScrollSwipe($Window) {
    [pscustomobject]@{
        X     = [int]($Window.Left + $Window.Width / 2)
        FromY = [int]($Window.Top + $Window.Height * 0.65)
        ToY   = [int]($Window.Top + $Window.Height * 0.35)
    }
}

# The first node for which `Test` is true, looked for up to $Retries times; $null if none.
function Find-Node([scriptblock] $Test, [int] $Tries = $Retries) {
    for ($i = 0; $i -lt $Tries; $i++) {
        $found = Get-Screen | Where-Object $Test | Select-Object -First 1
        if ($found) { return $found }
        Start-Sleep -Milliseconds $DelayMs
    }
    $null
}

function Invoke-Tap([string] $Name) {
    $node = Find-Node { $_.Name -ceq $Name }
    if (-not $node) { throw "no node named `"$Name`"" }
    Invoke-Adb shell input tap $node.X $node.Y | Out-Null
    Start-Sleep -Milliseconds $DelayMs
}

# The navigation list's swipe for an item it doesn't show: a finger drawn up from the lowest item
# shown to the highest (items are named "... Calculator" or "... Converter").
function Invoke-NavigationSwipe {
    $items = @(Get-Screen | Where-Object { $_.Name -cmatch ' (Calculator|Converter)$' })
    if ($items.Count -lt 2) { return $false }
    $from = $items | Sort-Object Y -Descending | Select-Object -First 1
    $to = $items | Sort-Object Y | Select-Object -First 1
    Invoke-Adb shell input swipe $from.X $from.Y $to.X $to.Y 400 | Out-Null
    Start-Sleep -Milliseconds ($DelayMs * 2)
    $true
}

function Assert-Node([scriptblock] $Test, [string] $What) {
    if (-not (Find-Node $Test)) {
        $names = (Get-Screen | ForEach-Object Name) -join ' | '
        throw "no node $What; the screen has: $names"
    }
}

# Runs one step; throws with the reason if it fails.
function Invoke-Step([string] $Op, $Arg) {
    switch -CaseSensitive ($Op) {
        'launch' {
            Invoke-Adb shell am force-stop $Package | Out-Null
            Invoke-Adb shell am start -W -n "$Package/android.app.NativeActivity" | Out-Null
            if (-not (Find-Node { $_.Name -ceq 'Open Navigation' } ($Retries * 3))) {
                throw 'the app showed no "Open Navigation" after its start'
            }
        }
        'open' {
            Invoke-Tap 'Open Navigation'
            for ($i = 0; $i -lt 4; $i++) {
                if (Find-Node { $_.Name -ceq $Arg } 2) { break }
                if (-not (Invoke-NavigationSwipe)) { break }
            }
            Invoke-Tap $Arg
        }
        'tap' { Invoke-Tap $Arg }
        'scroll_to' {
            # A finger drawn up the middle of the app's window until the node shows (Settings on
            # a low screen).
            for ($i = 0; $i -lt 6; $i++) {
                if (Find-Node { $_.Name -ceq $Arg } 2) { return }
                $swipe = Get-ScrollSwipe (Get-Window)
                Invoke-Adb shell input swipe $swipe.X $swipe.FromY $swipe.X $swipe.ToY 400 | Out-Null
                Start-Sleep -Milliseconds ($DelayMs * 2)
            }
            Assert-Node { $_.Name -ceq $Arg } "named `"$Arg`" after scrolling"
        }
        'expect_checked' { Assert-Node { $_.Name -ceq $Arg -and $_.Checked } "named `"$Arg`" that is checked" }
        'taps' { foreach ($name in $Arg) { Invoke-Tap $name } }
        'tap_if_present' {
            $node = Find-Node { $_.Name -ceq $Arg } 2
            if ($node) {
                Invoke-Adb shell input tap $node.X $node.Y | Out-Null
                Start-Sleep -Milliseconds $DelayMs
            }
        }
        'expect' { Assert-Node { $_.Name -ceq $Arg } "named `"$Arg`"" }
        'expect_prefix' { Assert-Node { $_.Name.StartsWith($Arg, [StringComparison]::Ordinal) } "named `"$Arg...`"" }
        'expect_contains' { Assert-Node { $_.Name.Contains($Arg) } "named `"...$Arg...`"" }
        'expect_absent' {
            Start-Sleep -Milliseconds $DelayMs
            if (Get-Screen | Where-Object { $_.Name -ceq $Arg }) { throw "a node named `"$Arg`"" }
        }
        'display' {
            $wanted = ConvertTo-Loose "Display is $Arg"
            Assert-Node { (ConvertTo-Loose $_.Name) -ceq $wanted } "`"Display is $Arg`""
        }
        'expression' {
            $wanted = ConvertTo-Loose "Expression is $Arg"
            Assert-Node { (ConvertTo-Loose $_.Name).StartsWith($wanted, [StringComparison]::Ordinal) } "`"Expression is $Arg`""
        }
        'edition' { }
        default { throw "unknown step `"$Op`"" }
    }
}

# --- the self-test: no device

if ($SelfTest) {
    $recordedDump = Join-Path $PSScriptRoot 'fixtures/window-dump.xml'
    $Package = 'com.rapalon.calccraft.a11ytest'
    $Retries = 1
    $DelayMs = 0
    $problems = [Collections.Generic.List[string]]::new()
    function Test-That([bool] $Condition, [string] $What) { if (-not $Condition) { $problems.Add($What) } }

    $nodes = @(Get-Screen)
    # The fixture's empty node ([0,0][0,0]), its nameless one and the other app's are left out.
    Test-That ($nodes.Count -eq 8) "the fixture gives $($nodes.Count) nodes, not 8: $(($nodes | ForEach-Object Name) -join ' | ')"
    $seven = Find-Node { $_.Name -ceq 'Number 7' }
    Test-That ($seven -and $seven.X -eq 830 -and $seven.Y -eq 300) "Number 7 isn't at (830, 300): $seven"
    Test-That ($null -eq (Find-Node { $_.Name -ceq 'Nameless' })) 'a node of another app was found'
    $wanted = ConvertTo-Loose 'Display is 1,000'
    Test-That ([bool](Find-Node { (ConvertTo-Loose $_.Name) -ceq $wanted })) 'Display is 1,000 matches no "Display is 1(U+2019)000"'
    Test-That ((ConvertTo-Loose '6.25') -ceq (ConvertTo-Loose '6,25')) '6.25 and 6,25 differ'
    try { Invoke-Step 'expect' 'Plus'; } catch { $problems.Add("expect Plus: $($_.Exception.Message)") }
    try { Invoke-Step 'expression' '12 + 7 × 3 ='; } catch { $problems.Add("expression: $($_.Exception.Message)") }
    try { Invoke-Step 'expect_contains' 'displaying 3 equations'; } catch { $problems.Add("graph: $($_.Exception.Message)") }
    $absent = $false
    try { Invoke-Step 'expect' 'Sine' } catch { $absent = $true }
    Test-That $absent 'expect Sine passed on a screen without it'
    # The fixture has no checked node (its `checked` attributes are all "false").
    Test-That (-not ($nodes | Where-Object Checked)) 'a node of the fixture reads as checked'
    $unchecked = $false
    try { Invoke-Step 'expect_checked' 'Plus' } catch { $unchecked = $true }
    Test-That $unchecked 'expect_checked Plus passed on an unchecked node'
    Test-That ((Get-Edition 'com.rapalon.calccraft.pro') -ceq 'pro') 'the Pro app is not the pro edition'
    foreach ($id in 'com.rapalon.calccraft', 'com.rapalon.calccraft.test', 'com.rapalon.calccraft.a11ytest') {
        Test-That ((Get-Edition $id) -ceq 'free') "$id is not the free edition"
    }

    # The scroll_to swipe stays inside the app's window in both orientations: the window is the
    # dump's outermost app node, not `wm size` (the portrait panel, which sent the swipe off a
    # phone held sideways).
    $window = Get-Window
    Test-That ($window.Width -eq 1080 -and $window.Height -eq 2340) "the portrait window isn't 1080 x 2340: $window"
    $swipe = Get-ScrollSwipe $window
    Test-That ($swipe.X -eq 540 -and $swipe.FromY -eq 1521 -and $swipe.ToY -eq 819) "the portrait swipe isn't (540, 1521 -> 819): $swipe"
    $recordedDump = Join-Path $PSScriptRoot 'fixtures/window-dump-landscape.xml'
    $landscape = @(Get-Screen)
    Test-That ($landscape.Count -eq 5) "the landscape fixture gives $($landscape.Count) nodes, not 5: $(($landscape | ForEach-Object Name) -join ' | ')"
    Test-That ([bool](Find-Node { $_.Name -ceq 'Classic' -and $_.Checked })) 'Classic is not checked in the landscape fixture'
    $window = Get-Window
    Test-That ($window.Left -eq 0 -and $window.Top -eq 0 -and $window.Width -eq 2340 -and $window.Height -eq 1080) "the landscape window isn't 2340 x 1080: $window"
    $swipe = Get-ScrollSwipe $window
    Test-That ($swipe.X -eq 1170 -and $swipe.FromY -eq 702 -and $swipe.ToY -eq 378) "the landscape swipe isn't (1170, 702 -> 378): $swipe"
    Test-That ($swipe.X -lt $window.Width -and $swipe.FromY -lt $window.Height -and $swipe.ToY -gt 0) "the landscape swipe leaves the window: $swipe"
    $recordedDump = Join-Path $PSScriptRoot 'fixtures/window-dump.xml'
    # The fallback without an app node: `wm size` turned by the dump's rotation.
    $turned = ConvertTo-WindowSize 'Physical size: 1080x2340' '1'
    Test-That ($turned[0] -eq 2340 -and $turned[1] -eq 1080) "wm size turned by rotation 1 isn't 2340 x 1080: $turned"
    $upright = ConvertTo-WindowSize 'Physical size: 1080x2340' '0'
    Test-That ($upright[0] -eq 1080 -and $upright[1] -eq 2340) "wm size at rotation 0 isn't 1080 x 2340: $upright"
    $turned = ConvertTo-WindowSize 'Physical size: 1080x2340' '3'
    Test-That ($turned[0] -eq 2340 -and $turned[1] -eq 1080) "wm size turned by rotation 3 isn't 2340 x 1080: $turned"

    $files = @(Get-ChildItem $scenarioDir -Filter *.json)
    Test-That ($files.Count -ge 6) "only $($files.Count) scenarios"
    foreach ($file in $files) {
        $json = Get-Content $file.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
        Test-That ($json.name -ceq $file.BaseName) "$($file.Name) is named $($json.name)"
        foreach ($step in $json.steps) {
            $properties = @($step.PSObject.Properties)
            Test-That ($properties.Count -eq 1) "$($file.Name): a step with $($properties.Count) keys"
            Test-That ($knownSteps -ccontains $properties[0].Name) "$($file.Name): an unknown step $($properties[0].Name)"
        }
    }
    if ($problems.Count) {
        $problems | ForEach-Object { Write-Host "FAIL $_" }
        exit 1
    }
    Write-Host "Self-test passed: the two recorded dumps and $($files.Count) scenarios."
    exit 0
}

# --- the run

if (-not $Serial) { throw 'Give the device with -Serial (adb devices lists them), or run -SelfTest.' }
$state =(& $Adb -s $Serial get-state 2>&1) -join ' '
if ($LASTEXITCODE -ne 0 -or $state -notmatch 'device') { throw "no device $Serial ($state)" }
$installed = (& $Adb -s $Serial shell pm path $Package 2>&1) -join ' '
if ($installed -notmatch 'package:') { throw "$Package isn't installed on $Serial" }

$files = Get-ChildItem $scenarioDir -Filter *.json | Sort-Object Name
if ($Scenario) { $files = $files | Where-Object { $Scenario -contains $_.BaseName } }
if (-not $files) { throw "no scenarios in $scenarioDir match $($Scenario -join ', ')" }

$rows = [Collections.Generic.List[object]]::new()
foreach ($file in $files) {
    $json = Get-Content $file.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
    $failed = $false
    $number = 0
    $skipping = $false
    foreach ($step in $json.steps) {
        $number++
        $property = @($step.PSObject.Properties)[0]
        $op = $property.Name
        $arg = $property.Value
        # `edition`: the steps up to the next one are only for that edition ("any": for all);
        # the others' steps leave no row.
        if ($op -ceq 'edition') {
            $skipping = $arg -cne 'any' -and $arg -cne (Get-Edition $Package)
            continue
        }
        if ($skipping) { continue }
        $shown = if ($arg -is [array]) { $arg -join ', ' } else { "$arg" }
        $row = [pscustomobject]@{
            Scenario = $json.name
            Step     = $number
            Action   = "$op $shown".Trim()
            Result   = 'SKIP'
            Detail   = ''
        }
        if (-not $failed) {
            try {
                Invoke-Step $op $arg
                $row.Result = 'PASS'
            } catch {
                $row.Result = 'FAIL'
                $row.Detail = $_.Exception.Message
                $failed = $true
            }
        }
        $rows.Add($row)
        Write-Host ('{0,-26} {1,3} {2,-4} {3}' -f $row.Scenario, $row.Step, $row.Result, $row.Action)
    }
}
Remove-Item $dumpHere -ErrorAction SilentlyContinue

$rows | Format-Table Scenario, Step, Result, Action, Detail -AutoSize -Wrap | Out-String -Width 200 | Write-Host
$summary = $rows | Group-Object Scenario | ForEach-Object {
    $bad = @($_.Group | Where-Object Result -ne 'PASS')
    [pscustomobject]@{ Scenario = $_.Name; Steps = $_.Count; Result = if ($bad) { 'FAIL' } else { 'PASS' } }
}
$summary | Format-Table -AutoSize | Out-String | Write-Host

if ($Out) {
    $device = (& $Adb -s $Serial shell getprop ro.product.model 2>$null) -join ''
    $version = ((& $Adb -s $Serial shell dumpsys package $Package 2>$null) -match 'versionName=' | Select-Object -First 1) -replace '.*versionName=', ''
    $lines = @(
        "CalcCraft device scenarios: $Package $version on $device ($Serial), $(Get-Date -Format 'yyyy-MM-dd HH:mm')",
        '',
        '| Scenario | Steps | Result |',
        '| --- | --- | --- |'
    )
    $lines += $summary | ForEach-Object { "| $($_.Scenario) | $($_.Steps) | $($_.Result) |" }
    $lines += '', '| Scenario | Step | Action | Result | Detail |', '| --- | --- | --- | --- | --- |'
    $lines += $rows | ForEach-Object { "| $($_.Scenario) | $($_.Step) | $($_.Action -replace '\|', '\|') | $($_.Result) | $($_.Detail -replace '\|', '\|') |" }
    $directory = Split-Path $Out
    if ($directory) { New-Item -ItemType Directory -Force $directory | Out-Null }
    [IO.File]::WriteAllLines($Out, $lines, [Text.UTF8Encoding]::new($false))
    Write-Host "Wrote $Out"
}

if ($rows | Where-Object Result -eq 'FAIL') { exit 1 }
exit 0
