[CmdletBinding()]
param([string]$Version=(Get-Date -Format 'yyyy.MM.dd'),[switch]$NoBuild)
# Builds the player release: dist/RatchetVR-<version>/ and its .zip. Players unzip it and double-click RatchetVR.exe
# (crates/rc-engine/src/bin/ratchet-vr.rs), which extracts their own disc once and starts the game in VR.
# Nothing from the disc is ever packaged: the folder is checked against its exact file list.
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$Root=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$Name="RatchetVR-$Version"
$Dist=Join-Path $Root 'dist'
$Out=Join-Path $Dist $Name
$Zip=Join-Path $Dist "$Name.zip"
$Target=Join-Path $Root 'target/release'
$OldRustFlags=$env:RUSTFLAGS
$OldCMakeGenerator=$env:CMAKE_GENERATOR
Push-Location $Root
try {
    if (-not $NoBuild) {
        foreach ($Tool in @('cargo','rustc','cmake')) { if (-not (Get-Command $Tool -ErrorAction SilentlyContinue)) {throw "Missing $Tool"} }
        # CMake otherwise picks VS 2022 even when only VS 2019 has the C++ workload (same choice as Build.ps1).
        if (-not $env:CMAKE_GENERATOR) {
            $VsWhere=Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
            if (Test-Path -LiteralPath $VsWhere) {
                $Vs=& $VsWhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion
                if ($Vs) {
                    switch (([version]$Vs).Major) {
                        16 {$env:CMAKE_GENERATOR='Visual Studio 16 2019'}
                        17 {$env:CMAKE_GENERATOR='Visual Studio 17 2022'}
                        18 {$env:CMAKE_GENERATOR='Visual Studio 18 2026'}
                    }
                }
            }
        }
        # Players' CPUs differ: never ship a target-cpu=native build.
        $env:RUSTFLAGS=''
        # Cargo writes progress to stderr even on success, which Windows PowerShell turns into errors; decide by exit code.
        $PreviousPreference=$ErrorActionPreference
        try {
            $ErrorActionPreference='Continue'
            & cargo build --release --locked -p rc-engine -p rc-extract --features rc-engine/vr --bins 2>&1|Out-Host
            $Code=$LASTEXITCODE
        } finally {$ErrorActionPreference=$PreviousPreference}
        if ($Code -ne 0) {throw "cargo build failed (exit $Code)"}
    }
    foreach ($Binary in @('rerac.exe','rerac-extract.exe','ratchet-vr.exe')) {
        if (-not (Test-Path -LiteralPath (Join-Path $Target $Binary))) {throw "Missing target/release/$Binary (run without -NoBuild)"}
    }
    if (Test-Path -LiteralPath $Out) {Remove-Item -LiteralPath $Out -Recurse -Force}
    if (Test-Path -LiteralPath $Zip) {Remove-Item -LiteralPath $Zip -Force}
    [IO.Directory]::CreateDirectory($Out)|Out-Null
    Copy-Item -LiteralPath (Join-Path $Target 'ratchet-vr.exe') -Destination (Join-Path $Out 'RatchetVR.exe')
    Copy-Item -LiteralPath (Join-Path $Target 'ratchet-vr.exe') -Destination (Join-Path $Out 'RatchetVR Sandbox.exe')
    Copy-Item -LiteralPath (Join-Path $Target 'rerac.exe') -Destination $Out
    Copy-Item -LiteralPath (Join-Path $Target 'rerac-extract.exe') -Destination $Out
    # App-local Visual C++ runtime (Microsoft's redistributable CRT files), so players need no separate installer.
    $VsWhere=Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    $Crt=@(& $VsWhere -products '*' -find 'VC\Redist\MSVC\*\x64\Microsoft.VC14*.CRT'|Where-Object {Test-Path -LiteralPath (Join-Path $_ 'vcruntime140_1.dll')}|
        Sort-Object {[version](Split-Path (Split-Path (Split-Path $_ -Parent) -Parent) -Leaf)} -Descending)
    if (-not $Crt) {throw 'No Visual C++ redistributable CRT folder found (install the C++ build tools).'}
    foreach ($Dll in @('vcruntime140.dll','vcruntime140_1.dll','msvcp140.dll')) {Copy-Item -LiteralPath (Join-Path $Crt[0] $Dll) -Destination $Out}
    Copy-Item -LiteralPath 'crates/rc-engine/assets' -Destination (Join-Path $Out 'assets') -Recurse
    Get-ChildItem -LiteralPath (Join-Path $Out 'assets') -Recurse -Force -File|Where-Object {$_.Name.StartsWith('.')}|Remove-Item -Force
    Copy-Item -LiteralPath 'LICENSE' -Destination (Join-Path $Out 'LICENSE.txt')
    Copy-Item -LiteralPath 'docs/vr/THIRD_PARTY_NOTICES.md' -Destination (Join-Path $Out 'THIRD_PARTY_NOTICES.md')
    $Readme=@"
Ratchet & Clank VR $Version

HOW TO PLAY
1. Turn on your headset and start SteamVR.
2. Double-click RatchetVR.exe.
3. The first time, pick your own Ratchet & Clank disc image (.iso, US version SCUS-97199).
   The game is copied from it once into the "data" folder (a few minutes, about 5 GB).

RatchetVR Sandbox.exe starts on Kerwan with every weapon, gadget and planet unlocked.
Your saves are in the "saves" folder. Keep this folder somewhere you can write to (Desktop, Documents, a games
folder), not in Program Files.

Controls and help: https://github.com/npiriou/Rerac-vr

Unofficial fan project, built on ReRAC (https://github.com/re-rac/rerac). Not affiliated with or endorsed by Sony
Interactive Entertainment, Insomniac Games or the ReRAC team. No game data is included: you need your own disc.
"@
    [IO.File]::WriteAllText((Join-Path $Out 'README.txt'),($Readme -replace "`r?`n","`r`n"),(New-Object Text.UTF8Encoding($false)))
    # The exact file list: binaries, shaders, texts. Anything else (game data, saves, logs) stops the release.
    $Allowed='^(RatchetVR\.exe|RatchetVR Sandbox\.exe|rerac\.exe|rerac-extract\.exe|vcruntime140\.dll|vcruntime140_1\.dll|msvcp140\.dll|README\.txt|LICENSE\.txt|THIRD_PARTY_NOTICES\.md|assets\\shaders\\[^\\]+\.wgsl)$'
    $Unexpected=@(Get-ChildItem -LiteralPath $Out -Recurse -Force -File|ForEach-Object {$_.FullName.Substring($Out.Length+1)}|Where-Object {$_ -notmatch $Allowed})
    if ($Unexpected) {throw "Unexpected files in the release folder: $($Unexpected -join ', ')"}
    Compress-Archive -Path $Out -DestinationPath $Zip
    $Size=[math]::Round((Get-Item -LiteralPath $Zip).Length/1MB,1)
    Write-Host "Release folder: $Out"
    Write-Host "Release zip: $Zip ($Size MB)"
} finally {$env:RUSTFLAGS=$OldRustFlags;$env:CMAKE_GENERATOR=$OldCMakeGenerator;Pop-Location}
