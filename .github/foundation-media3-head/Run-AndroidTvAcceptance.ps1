param(
    [string]$Serial = "",
    [Parameter(Mandatory=$true)][string]$ApkPath,
    [Parameter(Mandatory=$true)][string]$FixtureDirectory,
    [string]$Output = "$PSScriptRoot\results",
    [int]$PerFixtureSeconds = 25,
    [int]$SoakSeconds = 120
)
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $Output | Out-Null

$adb = Get-Command adb -ErrorAction Stop
$target = @()
if ($Serial) { $target = @("-s", $Serial) }

$deviceLines = & $adb @target devices -l
if (-not ($deviceLines | Select-String "\sdevice\s")) {
    throw "No authorised Android TV is reachable through adb."
}
if (-not (Test-Path -LiteralPath $ApkPath -PathType Leaf)) {
    throw "Candidate demo APK not found."
}
$fixtures = Get-ChildItem -LiteralPath $FixtureDirectory -Filter "flash-click-*.mp4" | Sort-Object Name
if ($fixtures.Count -ne 8) {
    throw "Expected exactly eight validated flash/click fixture files."
}

$package = "androidx.media3.demo.main"
$activity = "androidx.media3.demo.main/.PlayerActivity"
$remoteRoot = "/sdcard/Download/stremio-av-sync-acceptance"
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$dir = Join-Path $Output $stamp
New-Item -ItemType Directory -Force -Path $dir | Out-Null

& $adb @target install -r $ApkPath | Tee-Object -FilePath (Join-Path $dir "install.txt")
if ($LASTEXITCODE -ne 0) { throw "Candidate APK installation failed." }

# These permission grants are best effort only. Android versions and demo APK
# manifests differ in which storage permission is grantable. A rejected optional
# grant must not terminate otherwise valid hardware acceptance.
$previousErrorActionPreference = $ErrorActionPreference
try {
    $ErrorActionPreference = "Continue"
    & $adb @target shell pm grant $package android.permission.READ_MEDIA_VIDEO 2>$null | Out-Null
    & $adb @target shell pm grant $package android.permission.READ_EXTERNAL_STORAGE 2>$null | Out-Null
} finally {
    $ErrorActionPreference = $previousErrorActionPreference
}
& $adb @target shell mkdir -p $remoteRoot | Out-Null

foreach ($fixture in $fixtures) {
    $remote = "$remoteRoot/$($fixture.Name)"
    & $adb @target push $fixture.FullName $remote | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Failed to push fixture $($fixture.Name)." }
}

& $adb @target shell getprop > (Join-Path $dir "getprop.txt")
& $adb @target shell dumpsys display > (Join-Path $dir "display-baseline.txt")
& $adb @target shell dumpsys SurfaceFlinger --display-id > (Join-Path $dir "surfaceflinger-baseline.txt") 2>&1

$records = @()
foreach ($fixture in $fixtures) {
    $label = [IO.Path]::GetFileNameWithoutExtension($fixture.Name)
    $remote = "$remoteRoot/$($fixture.Name)"
    $caseDir = Join-Path $dir $label
    New-Item -ItemType Directory -Force -Path $caseDir | Out-Null

    & $adb @target shell am force-stop $package | Out-Null
    & $adb @target logcat -c
    & $adb @target shell dumpsys display > (Join-Path $caseDir "display-before.txt")
    & $adb @target shell am start -W -n $activity -a androidx.media3.demo.main.action.VIEW -d "file://$remote" --es repeat_mode ONE > (Join-Path $caseDir "launch.txt")
    if ($LASTEXITCODE -ne 0) { throw "Failed to launch $($fixture.Name)." }

    Start-Sleep -Seconds 5
    & $adb @target shell input keyevent 85 | Out-Null
    Start-Sleep -Seconds 2
    & $adb @target shell input keyevent 85 | Out-Null
    Start-Sleep -Seconds 3
    & $adb @target shell input keyevent 90 | Out-Null
    Start-Sleep -Seconds ([Math]::Max(1, $PerFixtureSeconds - 10))

    & $adb @target shell dumpsys display > (Join-Path $caseDir "display-after.txt")
    & $adb @target shell dumpsys SurfaceFlinger --display-id > (Join-Path $caseDir "surfaceflinger-after.txt") 2>&1
    & $adb @target logcat -d -v threadtime > (Join-Path $caseDir "logcat.txt")
    $records += [ordered]@{ fixture=$fixture.Name; completed=$true }
}

$soakFixture = $fixtures | Where-Object Name -eq "flash-click-23.976.mp4" | Select-Object -First 1
$soakDir = Join-Path $dir "soak-23.976"
New-Item -ItemType Directory -Force -Path $soakDir | Out-Null
& $adb @target shell am force-stop $package | Out-Null
& $adb @target logcat -c
& $adb @target shell am start -W -n $activity -a androidx.media3.demo.main.action.VIEW -d "file://$remoteRoot/$($soakFixture.Name)" --es repeat_mode ONE > (Join-Path $soakDir "launch.txt")
Start-Sleep -Seconds $SoakSeconds
& $adb @target shell dumpsys display > (Join-Path $soakDir "display-after.txt")
& $adb @target shell dumpsys SurfaceFlinger --display-id > (Join-Path $soakDir "surfaceflinger-after.txt") 2>&1
& $adb @target logcat -d -v threadtime > (Join-Path $soakDir "logcat.txt")
& $adb @target shell am force-stop $package | Out-Null

$identity = if ($Serial) { $Serial } else { "default" }
$hash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($identity))).Substring(0,16)
$summary = [ordered]@{
    schemaVersion = 2
    capturedAtUtc = (Get-Date).ToUniversalTime().ToString("o")
    deviceSerialHashed = $hash
    fixtureCases = $records
    soakSeconds = $SoakSeconds
    displayEvidenceCaptured = $true
    physicalMeasurementCompleted = $false
    cameraAnalysisCompleted = $false
    productionStremioModified = $false
    note = "Candidate Media3 demo APK only. Physical output acceptance still requires camera flash/click analysis."
}
$summary | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 (Join-Path $dir "summary.json")
Write-Host "Unattended Android TV acceptance evidence captured: $dir"
