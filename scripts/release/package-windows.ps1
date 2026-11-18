param([Parameter(Mandatory)] [string] $Destination)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$version = node (Join-Path $PSScriptRoot 'version.mjs')
$target = 'x86_64-pc-windows-msvc'
$build = Join-Path $repo "src-tauri/target/$target/release"
$tauri = Join-Path $repo 'node_modules/.bin/tauri.cmd'
$Destination = [IO.Path]::GetFullPath($Destination)
$null = New-Item -ItemType Directory -Path $Destination -Force

Push-Location $repo
try {
    & $tauri build --ci --target $target --no-bundle -- --locked
    # Save the portable executable before the bundler marks the installed copies.
    $portable = Join-Path $build 'codedoctor.exe'
    if ((Get-Item -LiteralPath $portable).VersionInfo.ProductVersion -ne $version) {
        throw 'Unexpected packaged Windows executable version'
    }
    Copy-Item -LiteralPath $portable -Destination $Destination

    & $tauri bundle --ci --target $target --bundles nsis,msi
    Copy-Item -LiteralPath (Join-Path $build "bundle/nsis/CodeDoctor_${version}_x64-setup.exe") -Destination $Destination
    Copy-Item -LiteralPath (Join-Path $build "bundle/msi/CodeDoctor_${version}_x64_en-US.msi") -Destination $Destination
} finally {
    Pop-Location
}
