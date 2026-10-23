$ErrorActionPreference='Stop'
Write-Host 'ReRAC original-game VR environment check (no software is installed and nothing is uploaded)'
foreach ($Tool in @('git','gh','rustc','cargo','cmake')) {
    $Found=Get-Command $Tool -ErrorAction SilentlyContinue
    if ($Found) {Write-Host "$Tool : $($Found.Source)"} else {Write-Warning "Missing: $Tool"}
}
if (Get-Command rustc -ErrorAction SilentlyContinue) {& rustc -vV}
if (Get-Command cargo -ErrorAction SilentlyContinue) {& cargo --version}
foreach ($Key in @('HKLM:\SOFTWARE\Khronos\OpenXR\1','HKCU:\SOFTWARE\Khronos\OpenXR\1')) {
    if (Test-Path $Key) {Get-ItemProperty $Key -Name ActiveRuntime -ErrorAction SilentlyContinue|Select-Object ActiveRuntime}
}
try {Get-CimInstance Win32_VideoController|Select-Object Name,DriverVersion} catch {Write-Warning 'GPU query unavailable.'}
Write-Host 'Expected: native Windows MSVC Rust target, C++ build tools + Windows SDK, CMake, PICO Connect, SteamVR as active OpenXR runtime.'
Write-Host 'Without a headset, use Run.ps1 -DataDir <extracted-folder> -DesktopPreview (normal boot), or add -Mode debug for Kerwan. Tracking and controller checks remain pending until hardware is connected.'
