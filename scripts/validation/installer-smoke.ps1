param([Parameter(Mandatory)] [string] $Folder)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
if ($env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Installer smoke test requires an isolated GitHub-hosted Windows runner'
}

$version = node scripts/release/version.mjs
$folderPath = (Resolve-Path -LiteralPath $Folder).Path
$scratch = Join-Path $env:RUNNER_TEMP 'codedoctor-installer-smoke'
$installPath = Join-Path $scratch 'nsis'
$null = New-Item -ItemType Directory -Path $scratch
$baseline = Join-Path $scratch 'CodeDoctor_0.1.1_x64-setup.exe'
Invoke-WebRequest -Uri 'https://github.com/yuchen0x1/CodeDoctor/releases/download/v0.1.1/CodeDoctor_0.1.1_x64-setup.exe' -OutFile $baseline -TimeoutSec 60
if ((Get-FileHash -LiteralPath $baseline).Hash -ine 'BEF71936A25A12D493AA2CD62F12AF7E5BFD97D42C6AF9699B6BC176070DB13A') {
    throw 'Unexpected baseline installer digest'
}

function Install-Nsis([string] $Package) {
    $process = Start-Process -FilePath $Package -ArgumentList "/S /D=$installPath" -WindowStyle Hidden -PassThru -Wait
    if ($process.ExitCode -ne 0) { throw "NSIS exited with $($process.ExitCode)" }
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

Install-Nsis $baseline
$installed = Join-Path $installPath 'codedoctor.exe'
Test-Launch $installed '0.1.1'
$database = Join-Path $env:APPDATA 'com.codedoctor.desktop/codedoctor.db'
python -c 'import sqlite3, sys; db=sqlite3.connect(sys.argv[1]); db.execute("create table upgrade_test(value integer)"); db.execute("insert into upgrade_test values (42)"); db.commit()' $database

Install-Nsis (Join-Path $folderPath "CodeDoctor_${version}_x64-setup.exe")
Test-Launch $installed $version
python -c 'import sqlite3, sys; db=sqlite3.connect(sys.argv[1]); assert db.execute("select value from upgrade_test").fetchone() == (42,); assert db.execute("pragma integrity_check").fetchone() == ("ok",)' $database

$uninstall = Start-Process -FilePath (Join-Path $installPath 'uninstall.exe') -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
if ($uninstall.ExitCode -ne 0 -or (Test-Path -LiteralPath $installed)) { throw 'NSIS uninstall did not complete' }
if (!(Test-Path -LiteralPath $database)) { throw 'Uninstall removed user data' }
'Verified NSIS clean installation, v0.1.1 upgrade, retained SQLite data and uninstall.'
