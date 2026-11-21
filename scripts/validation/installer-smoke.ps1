param(
    [Parameter(Mandatory)] [string] $Folder,
    [ValidateSet('nsis', 'msi')] [string] $Installer = 'nsis'
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
if ($env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Installer smoke test requires an isolated GitHub-hosted Windows runner'
}

$version = node scripts/release/version.mjs
$folderPath = (Resolve-Path -LiteralPath $Folder).Path
$scratch = Join-Path $env:RUNNER_TEMP "codedoctor-installer-smoke-$Installer"
$installPath = Join-Path $scratch $Installer
$null = New-Item -ItemType Directory -Path $scratch
$baselineFolder = Join-Path $scratch 'baseline'
$null = New-Item -ItemType Directory -Path $baselineFolder
$suffix = if ($Installer -eq 'msi') { 'x64_en-US.msi' } else { 'x64-setup.exe' }
$baseline = Join-Path $baselineFolder "CodeDoctor_0.1.1_$suffix"
$current = Join-Path $folderPath "CodeDoctor_${version}_$suffix"
$digest = if ($Installer -eq 'msi') {
    '20DD046473908DEE01FADBCD033EC0B9BAAAA4D0D151B99B6C475CDEC03A8F9B'
} else {
    'BEF71936A25A12D493AA2CD62F12AF7E5BFD97D42C6AF9699B6BC176070DB13A'
}
Invoke-WebRequest -Uri "https://github.com/yuchen0x1/CodeDoctor/releases/download/v0.1.1/CodeDoctor_0.1.1_$suffix" -OutFile $baseline -TimeoutSec 60
if ((Get-FileHash -LiteralPath $baseline).Hash -ine $digest) {
    throw 'Unexpected baseline installer digest'
}

function Invoke-Msi([string] $Operation, [string] $Package) {
    $log = Join-Path $scratch "$Operation-$([IO.Path]::GetFileName($Package)).log"
    $arguments = "/$Operation `"$Package`" /qn /norestart INSTALLDIR=`"$installPath`" /L*v `"$log`""
    $process = Start-Process -FilePath (Join-Path $env:SystemRoot 'System32/msiexec.exe') -ArgumentList $arguments -PassThru -Wait
    if ($process.ExitCode -notin @(0, 3010)) { throw "MSI exited with $($process.ExitCode); see $log" }
}

function Install-Package([string] $Package) {
    if ($Installer -eq 'msi') {
        Invoke-Msi 'i' $Package
    } else {
        $process = Start-Process -FilePath $Package -ArgumentList "/S /D=$installPath" -WindowStyle Hidden -PassThru -Wait
        if ($process.ExitCode -ne 0) { throw "NSIS exited with $($process.ExitCode)" }
    }
}

function Test-DatabaseReady([string] $Database, [bool] $RequireRecovery, [int] $MinimumSchemaVersion) {
    if (!(Test-Path -LiteralPath $Database -PathType Leaf)) { return $false }
    $PSNativeCommandUseErrorActionPreference = $false
    python -I -c @'
import sqlite3, sys
from pathlib import Path

uri = Path(sys.argv[1]).resolve().as_uri() + "?mode=ro"
try:
    with sqlite3.connect(uri, uri=True, timeout=0.25) as db:
        version = db.execute("SELECT MAX(version) FROM schema_metadata").fetchone()[0]
        ready = version is not None and version >= int(sys.argv[3])
        if ready and sys.argv[2] == "1":
            recovered = db.execute(
                "SELECT status, completed_at IS NOT NULL, failure_message, config FROM scan_runs WHERE id = ?",
                ("installer-smoke-scan",),
            ).fetchone()
            import json
            ready = (
                recovered is not None
                and recovered[:3] == (
                    "failed", 1,
                    "scan was interrupted before completion; the original failure reason is unavailable. Please retry the scan.",
                )
                and json.loads(recovered[3]) == {
                    "maxLineLength": 91, "enabledAnalyzers": ["ruff"], "interrupted": True,
                }
            )
except sqlite3.OperationalError:
    sys.exit(1)
sys.exit(0 if ready else 1)
'@ $Database ([int] $RequireRecovery) $MinimumSchemaVersion
    if ($LASTEXITCODE -notin @(0, 1)) { throw "SQLite readiness probe exited with $LASTEXITCODE" }
    return $LASTEXITCODE -eq 0
}

function Test-Launch([string] $Executable, [string] $ExpectedVersion, [string] $Database, [switch] $RequireRecovery, [int] $MinimumSchemaVersion = 2) {
    if ((Get-Item -LiteralPath $Executable).VersionInfo.ProductVersion -ne $ExpectedVersion) {
        throw "Unexpected executable version at $Executable"
    }
    $process = Start-Process -FilePath $Executable -PassThru
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $windowReady = $false
    try {
        while ($timer.Elapsed.TotalSeconds -lt 30) {
            $process.Refresh()
            if ($process.HasExited) { throw "CodeDoctor $ExpectedVersion exited during launch ($($process.ExitCode))" }
            $windowReady = $process.MainWindowTitle -eq 'CodeDoctor'
            # Tauri creates the window before the setup callback opens and migrates SQLite.
            if ($windowReady -and (Test-DatabaseReady $Database $RequireRecovery.IsPresent $MinimumSchemaVersion) -and !$process.HasExited) { return }
            Start-Sleep -Milliseconds 500
        }
        if (!$windowReady) { throw "CodeDoctor $ExpectedVersion window did not appear" }
        throw "CodeDoctor $ExpectedVersion did not initialize a readable SQLite database at $Database"
    } finally {
        if (!$process.HasExited) { $process.Kill() }
        $process.WaitForExit()
    }
}

# Use the Windows known folder, as Tauri does for app_data_dir().
$appData = [Environment]::GetFolderPath([Environment+SpecialFolder]::ApplicationData)
if (!$appData) { throw 'Windows Roaming AppData is unavailable' }
$database = Join-Path $appData 'com.codedoctor.desktop/codedoctor.db'
if (Test-Path -LiteralPath $database) { throw 'Installer smoke test requires a clean CodeDoctor profile' }

Install-Package $baseline
$installed = Join-Path $installPath 'codedoctor.exe'
Test-Launch $installed '0.1.1' $database
# A running scan must be recovered by startup; an unchanged old database cannot pass the new launch check.
python -I -c @'
import sqlite3, sys
from pathlib import Path

uri = Path(sys.argv[1]).resolve().as_uri() + "?mode=rw"
with sqlite3.connect(uri, uri=True) as db:
    db.execute("INSERT INTO projects (id, normalized_path, name, created_at) VALUES (?, ?, ?, ?)",
               ("installer-smoke-project", sys.argv[2], "Installer smoke test", "1"))
    db.execute("INSERT INTO scan_runs (id, project_id, status, started_at, config, rule_version, analyzer_versions) VALUES (?, ?, ?, ?, ?, ?, ?)",
               ("installer-smoke-scan", "installer-smoke-project", "running", "1", '{"maxLineLength":91,"enabledAnalyzers":["ruff"]}', "smoke-test", "{}"))
    db.execute("CREATE TABLE upgrade_test (value INTEGER)")
    db.execute("INSERT INTO upgrade_test VALUES (42)")
'@ $database $scratch

Install-Package $current
Test-Launch $installed $version $database -RequireRecovery -MinimumSchemaVersion 3
python -I -c 'import sqlite3, sys; from pathlib import Path; db=sqlite3.connect(Path(sys.argv[1]).resolve().as_uri() + "?mode=ro", uri=True); assert db.execute("select value from upgrade_test").fetchone() == (42,); assert db.execute("pragma integrity_check").fetchone() == ("ok",)' $database

if ($Installer -eq 'msi') {
    Invoke-Msi 'x' $current
} else {
    $uninstall = Start-Process -FilePath (Join-Path $installPath 'uninstall.exe') -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
    if ($uninstall.ExitCode -ne 0) { throw 'NSIS uninstall did not complete' }
}
if (Test-Path -LiteralPath $installed) { throw "$Installer uninstall did not remove the executable" }
if (!(Test-Path -LiteralPath $database)) { throw 'Uninstall removed user data' }
"Verified $Installer clean installation, v0.1.1 upgrade, retained SQLite data and uninstall."
