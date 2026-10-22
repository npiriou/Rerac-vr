[CmdletBinding()]
param([ValidateSet('calibration','novalis')][string]$Scene='calibration',[string]$DataDir,
    [ValidateRange(0.1,10.0)][double]$UnitsPerMetre=1.0,[string]$SpawnGame,[switch]$Release)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$Root=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$Profile=if ($Release) {'release'} else {'debug'}
$Exe=Join-Path $Root "target/$Profile/rerac.exe"
if (-not (Test-Path -LiteralPath $Exe)) {throw 'No matching executable. Run scripts/vr/Build.ps1 first (add -Release for a release build).'}
$StatePath=Join-Path $Root "work/vr/last-build-$Profile.json"
if (-not (Test-Path -LiteralPath $StatePath)) {throw 'No successful build record for this profile. Run scripts/vr/Build.ps1 first.'}
$BuildState=Get-Content -LiteralPath $StatePath -Raw|ConvertFrom-Json
if ($BuildState.executable_sha256 -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $Exe).Hash) {
    throw 'Executable does not match the successful build record. Rebuild before testing.'
}
if ($Scene -eq 'novalis' -and (-not $DataDir -or -not (Test-Path -LiteralPath $DataDir -PathType Container))) {
    throw 'Novalis requires -DataDir pointing to your locally extracted, supported RAC1 data. Calibration does not need game data.'
}
$Out=Join-Path $Root 'work/vr';[IO.Directory]::CreateDirectory($Out)|Out-Null
$Log=Join-Path $Out ("$Scene-"+(Get-Date -Format 'yyyyMMdd-HHmmss')+'.log')
$Names=@('RC_VR','RC_VR_SCENE','RC_VR_UNITS_PER_METRE','RC_VR_SPAWN_GAME','RC_LEVEL','RC_AUDIO')
$Old=@{};foreach ($Name in $Names) {$Old[$Name]=[Environment]::GetEnvironmentVariable($Name,'Process')}
Push-Location $Root
try {
    $env:RC_VR='1';$env:RC_VR_SCENE=$Scene;$env:RC_LEVEL='1';$env:RC_AUDIO='0'
    $env:RC_VR_UNITS_PER_METRE=$UnitsPerMetre.ToString([Globalization.CultureInfo]::InvariantCulture)
    [Environment]::SetEnvironmentVariable('RC_VR_SPAWN_GAME',$SpawnGame,'Process')
    $Arguments=@();if ($Scene -eq 'novalis') {$Arguments=@('--data-dir',(Resolve-Path -LiteralPath $DataDir).Path)}
    Write-Host "Launch PICO Connect and SteamVR before continuing. Log: $Log"
    Write-Host 'The desktop window may be blank; use SteamVR VR View to inspect the headset output.'
    @('VR_POC launch record',('current_git_head='+(& git rev-parse HEAD)),('executable_sha256='+$BuildState.executable_sha256),('build_record='+$StatePath))|Set-Content -LiteralPath $Log -Encoding UTF8
    $PreviousPreference=$ErrorActionPreference
    try {
        $ErrorActionPreference='Continue'
        & $Exe @Arguments 2>&1|Tee-Object -FilePath $Log -Append|Out-Host
        $Code=$LASTEXITCODE
    } finally {$ErrorActionPreference=$PreviousPreference}
    if ($Code -ne 0) {throw "VR application exited with code $Code. See $Log"}
} finally {
    foreach ($Name in $Names) {[Environment]::SetEnvironmentVariable($Name,$Old[$Name],'Process')}
    Pop-Location
}
