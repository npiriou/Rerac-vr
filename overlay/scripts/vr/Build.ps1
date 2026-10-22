[CmdletBinding()]
param([switch]$Release,[switch]$RunTests,[switch]$SkipUpstreamQuickTests)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$Root=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$Out=Join-Path $Root 'work/vr'
[IO.Directory]::CreateDirectory($Out)|Out-Null
$Log=Join-Path $Out ("build-"+(Get-Date -Format 'yyyyMMdd-HHmmss')+'.log')
$Profile=if ($Release) {'release'} else {'debug'}
$StatePath=Join-Path $Out ("last-build-"+$Profile+'.json')
# A failed rebuild must never leave a success marker for a stale binary.
if (Test-Path -LiteralPath $StatePath) {Remove-Item -LiteralPath $StatePath}
$Steps=New-Object System.Collections.Generic.List[string]
$OldCMakeGenerator=$env:CMAKE_GENERATOR
$OldTestFeatures=$env:RC_TEST_FEATURES
function Step([string]$Name,[string]$Program,[string[]]$Arguments) {
    Write-Host "== $Name =="
    $PreviousPreference=$ErrorActionPreference
    try {
        # Windows PowerShell treats redirected native stderr as ErrorRecord objects.
        # Cargo writes progress to stderr even on success; decide by exit code.
        $ErrorActionPreference='Continue'
        & $Program @Arguments 2>&1 | Tee-Object -FilePath $Log -Append | Out-Host
        $Code=$LASTEXITCODE
    } finally {$ErrorActionPreference=$PreviousPreference}
    if ($Code -ne 0) {throw "$Name failed (exit $Code); see $Log"}
    $Steps.Add($Name)
}
Push-Location $Root
try {
    foreach ($Tool in @('cargo','rustc','cmake')) { if (-not (Get-Command $Tool -ErrorAction SilentlyContinue)) {throw "Missing $Tool"} }
    # CMake otherwise picks VS 2022 even when only VS 2019 has the C++ workload.
    if (-not $env:CMAKE_GENERATOR) {
        $VsWhere=Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
        if (Test-Path -LiteralPath $VsWhere) {
            $Version=& $VsWhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion
            if ($Version) {
                switch (([version]$Version).Major) {
                    16 {$env:CMAKE_GENERATOR='Visual Studio 16 2019'}
                    17 {$env:CMAKE_GENERATOR='Visual Studio 17 2022'}
                    18 {$env:CMAKE_GENERATOR='Visual Studio 18 2026'}
                }
                Write-Host "OpenXR CMake generator: $env:CMAKE_GENERATOR"
            }
        }
    }
    # Tests are opt-in: AGENTS.md requires an explicit user request before running any.
    if ($RunTests) {
        $Test=Join-Path $Out 'vr-core-tests.exe'
        Step 'Compile standalone VR core tests' 'rustc' @('--edition=2021','--test','crates/rc-engine/src/vr/core.rs','-o',$Test)
        Step 'Run standalone VR core tests' $Test @('--nocapture')
    }
    Step 'Check unchanged non-VR build path' 'cargo' @('check','--locked','-p','rc-engine')
    if ($RunTests -and -not $SkipUpstreamQuickTests) {
        $env:RC_TEST_FEATURES='rc-engine/vr'
        Step 'Upstream and VR game unit tests' 'cargo' @('xtask','test-quick')
        $env:RC_TEST_FEATURES=$OldTestFeatures
    }
    Step 'Check VR integration' 'cargo' @('check','--locked','-p','rc-engine','--features','vr')
    $BuildArgs=@('build','--locked','-p','rc-engine','--features','vr')
    if ($Release) {$BuildArgs+='--release'}
    Step 'Build VR executable' 'cargo' $BuildArgs
    $Exe=Join-Path $Root "target/$Profile/rerac.exe"
    if (-not (Test-Path -LiteralPath $Exe)) {throw 'Build exited successfully but the expected Windows executable is absent.'}
    $State=[ordered]@{profile=$Profile;executable_sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $Exe).Hash;completed_utc=(Get-Date).ToUniversalTime().ToString('o');steps_passed=@($Steps.ToArray());
        headset_tested=$false;gpu_budget_measured=$false;git_worktree_dirty=[bool](& git status --porcelain);rustc=(& rustc --version);commit=(& git rev-parse HEAD);}
    $State|ConvertTo-Json -Depth 4|Set-Content -LiteralPath $StatePath -Encoding UTF8
    Write-Host "Build checks passed. Tests requested: $RunTests. Log: $Log"
} finally {$env:CMAKE_GENERATOR=$OldCMakeGenerator;$env:RC_TEST_FEATURES=$OldTestFeatures;Pop-Location}
