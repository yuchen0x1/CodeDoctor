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

function Test-Launch([string] $Executable, [string] $ExpectedVersion) {
    if ((Get-Item -LiteralPath $Executable).VersionInfo.ProductVersion -ne $ExpectedVersion) {
        throw "Unexpected executable version at $Executable"
    }
    $process = Start-Process -FilePath $Executable -PassThru
    try {
        for ($i = 0; $i -lt 60; $i++) {
            $process.Refresh()
            if ($process.HasExited) { throw "CodeDoctor exited during launch ($($process.ExitCode))" }
            if ($process.MainWindowTitle -eq 'CodeDoctor') { return }
            Start-Sleep -Milliseconds 500
        }
        throw 'CodeDoctor window did not appear'
    } finally {
        if (!$process.HasExited) { $process.Kill(); $process.WaitForExit() }
    }
}

Install-Package $baseline
$installed = Join-Path $installPath 'codedoctor.exe'
Test-Launch $installed '0.1.1'
$database = Join-Path $env:APPDATA 'com.codedoctor.desktop/codedoctor.db'
python -I -c 'import sqlite3, sys; db=sqlite3.connect(sys.argv[1]); db.execute("create table upgrade_test(value integer)"); db.execute("insert into upgrade_test values (42)"); db.commit()' $database

Install-Package $current
Test-Launch $installed $version
python -I -c 'import sqlite3, sys; db=sqlite3.connect(sys.argv[1]); assert db.execute("select value from upgrade_test").fetchone() == (42,); assert db.execute("pragma integrity_check").fetchone() == ("ok",)' $database

if ($Installer -eq 'msi') {
    Invoke-Msi 'x' $current
} else {
    $uninstall = Start-Process -FilePath (Join-Path $installPath 'uninstall.exe') -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
    if ($uninstall.ExitCode -ne 0) { throw 'NSIS uninstall did not complete' }
}
if (Test-Path -LiteralPath $installed) { throw "$Installer uninstall did not remove the executable" }
if (!(Test-Path -LiteralPath $database)) { throw 'Uninstall removed user data' }
"Verified $Installer clean installation, v0.1.1 upgrade, retained SQLite data and uninstall."
