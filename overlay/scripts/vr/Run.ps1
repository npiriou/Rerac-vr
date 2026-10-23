[CmdletBinding()]
param([ValidateSet('normal','debug','debug-gold')][string]$Mode='normal',
    [ValidateSet('calibration','novalis','kerwan')][string]$Scene='kerwan',[string]$DataDir,
    [ValidateRange(0.1,10.0)][double]$UnitsPerMetre=1.0,[string]$SpawnGame,[switch]$Release,
    [switch]$DesktopPreview,[ValidateRange(0,600)][int]$SmokeTestSeconds=0)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
if ($SmokeTestSeconds -gt 0 -and -not $DesktopPreview) {throw '-SmokeTestSeconds requires -DesktopPreview.'}
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
if ($Scene -ne 'calibration' -and (-not $DataDir -or -not (Test-Path -LiteralPath $DataDir -PathType Container))) {
    throw 'Gameplay requires -DataDir pointing to your locally extracted, supported RAC1 data. Calibration does not need game data.'
}
$Out=Join-Path $Root 'work/vr';[IO.Directory]::CreateDirectory($Out)|Out-Null
$Surface=if ($DesktopPreview) {'preview'} else {'headset'}
$BootScene=if ($Scene -eq 'calibration') {'calibration'} elseif ($Mode -eq 'normal') {'frontend'} else {$Scene}
$Log=Join-Path $Out ("$Mode-$BootScene-$Surface-"+(Get-Date -Format 'yyyyMMdd-HHmmss')+'.log')
$Names=@('RC_VR_MODE','RC_SAVE_DIR','RC_GIVE_ITEMS','RC_GIVE_ITEMS_EQUIP','RC_GIVE_HYDROPACK','RC_GIVE_BOLTS','RC_GIVE_GOLD_BOLTS','RC_GIVE_GOLD_WEAPONS','RC_GAME_BEATEN','RC_SKIP_LOGOS','RC_PLAY_FLY','RC_VR','RC_VR_SCENE','RC_VR_UNITS_PER_METRE','RC_VR_SPAWN_GAME','RC_LEVEL','RC_UNLOCK_PLANETS','RC_AUDIO','RC_VR_DESKTOP_PREVIEW','RC_VR_PREVIEW_SECONDS','RC_FRONTEND','RC_SCENE','RC_PLAY','RC_FRAME_CAP','RC_OCCL','RC_MSAA','RC_SCREENSHOT','RC_SCREENSHOT_FRAME')
$Old=@{};foreach ($Name in $Names) {$Old[$Name]=[Environment]::GetEnvironmentVariable($Name,'Process')}
Push-Location $Root
try {
    # Empty optional environment variables must be absent, not empty: upstream treats RC_SCREENSHOT presence as a capture-and-exit request.
    foreach ($CaptureName in @('RC_SCREENSHOT','RC_SCREENSHOT_FRAME')) {
        if ([string]::IsNullOrEmpty([Environment]::GetEnvironmentVariable($CaptureName,'Process'))) {
            Remove-Item -LiteralPath ('Env:'+ $CaptureName) -ErrorAction SilentlyContinue
        }
    }
    $env:RC_VR='1';$env:RC_VR_MODE=$Mode;$env:RC_VR_SCENE=$Scene
    $env:RC_SAVE_DIR=Join-Path $Out ("saves/"+$Mode)
    if ($Mode -eq 'normal') {
        Remove-Item -LiteralPath Env:RC_LEVEL -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath Env:RC_UNLOCK_PLANETS -ErrorAction SilentlyContinue
        $env:RC_FRONTEND='1'
    } else {
        $env:RC_LEVEL=if ($Scene -eq 'kerwan') {'3'} else {'1'}
        $env:RC_UNLOCK_PLANETS=(0..18) -join ','
        $env:RC_FRONTEND='0'
    }
    $env:RC_VR_DESKTOP_PREVIEW=if ($DesktopPreview) {'1'} else {'0'}
    $env:RC_VR_PREVIEW_SECONDS=if ($SmokeTestSeconds -gt 0) {$SmokeTestSeconds.ToString()} else {$null}
    $env:RC_VR_UNITS_PER_METRE=$UnitsPerMetre.ToString([Globalization.CultureInfo]::InvariantCulture)
    if ([string]::IsNullOrWhiteSpace($SpawnGame)) {
        Remove-Item -LiteralPath Env:RC_VR_SPAWN_GAME -ErrorAction SilentlyContinue
    } else {
        [Environment]::SetEnvironmentVariable('RC_VR_SPAWN_GAME',$SpawnGame,'Process')
    }
    $Arguments=@();if ($Scene -ne 'calibration') {$Arguments=@('--data-dir',(Resolve-Path -LiteralPath $DataDir).Path)}
    Write-Host "Launch mode: $Mode. View: $Surface. Boot: $BootScene. Log: $Log"
    if ($DesktopPreview) {Write-Host 'Original ReRAC game with two offscreen stereo views; keyboard/gamepad playable. No headset simulation.'}
    else {Write-Host 'Connect the headset and launch PICO Connect/SteamVR. Use SteamVR VR View for headset output.'}
    @('VR_GAME launch record',('launch_mode='+$Mode),('save_dir='+$env:RC_SAVE_DIR),('frontend='+$env:RC_FRONTEND),('scene='+$BootScene),('boot_level='+$env:RC_LEVEL),('unlocked_planets='+$env:RC_UNLOCK_PLANETS),('current_git_head='+(& git rev-parse HEAD)),('executable_sha256='+$BuildState.executable_sha256),('build_record='+$StatePath))|Set-Content -LiteralPath $Log -Encoding UTF8
    $PreviousPreference=$ErrorActionPreference
    try {
        $ErrorActionPreference='Continue'
        & $Exe @Arguments 2>&1|Tee-Object -FilePath $Log -Append|Out-Host
        $Code=$LASTEXITCODE
    } finally {$ErrorActionPreference=$PreviousPreference}
    if ($Code -ne 0) {throw "VR application exited with code $Code. See $Log"}
} finally {
    foreach ($Name in $Names) {
        if ($null -eq $Old[$Name]) {Remove-Item -LiteralPath ('Env:'+ $Name) -ErrorAction SilentlyContinue}
        else {[Environment]::SetEnvironmentVariable($Name,$Old[$Name],'Process')}
    }
    Pop-Location
}
