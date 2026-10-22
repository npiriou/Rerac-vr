[CmdletBinding()]
param(
    [string]$Destination = (Join-Path $HOME 'source\rerac-vr-poc'),
    [string]$Owner = 'npiriou',
    [string]$RepositoryName = 'rerac-vr-poc',
    [switch]$PrepareOnly,
    [switch]$PublishSourceOnly
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
function Invoke-Checked([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed with exit code $LASTEXITCODE" }
}
foreach ($Tool in @('git','powershell')) {
    if (-not (Get-Command $Tool -ErrorAction SilentlyContinue)) { throw "Missing required program: $Tool" }
}
if ($RepositoryName -notmatch '^[A-Za-z0-9][A-Za-z0-9_.-]{0,99}$') { throw 'Invalid repository name.' }
if (Test-Path -LiteralPath $Destination) { throw 'Destination already exists. Choose a NEW empty destination; this script never overwrites a project.' }
if (-not $PrepareOnly) {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { throw 'Install GitHub CLI, then authenticate with gh auth login. Do not paste a token into chat.' }
    $Login = (& gh api user --jq .login).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'GitHub CLI is not authenticated. Run gh auth login locally, then retry.' }
    if ($Login -ine $Owner) { throw "Authenticated as $Login, not $Owner. No repository will be created." }
}
if (-not $PublishSourceOnly) {
    foreach ($Tool in @('cargo','rustc','cmake')) {
        if (-not (Get-Command $Tool -ErrorAction SilentlyContinue)) { throw "Missing $Tool. Install the documented build tools before running the build-gated bootstrap." }
    }
}
$Manifest = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'patch-manifest.json') -Raw | ConvertFrom-Json
[IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($Destination))) | Out-Null
Invoke-Checked 'git' @('-c','core.autocrlf=false','clone','--no-checkout','--origin','upstream','https://github.com/re-rac/rerac.git',$Destination)
Push-Location $Destination
try {
    Invoke-Checked 'git' @('config','core.autocrlf','false')
    Invoke-Checked 'git' @('checkout','-B','main',[string]$Manifest.upstreamCommit)
    & (Join-Path $PSScriptRoot 'Apply-Overlay.ps1') -Repository (Get-Location).Path
    if (-not $PublishSourceOnly) {
        & '.\scripts\vr\Build.ps1'
    } else {
        Write-Warning 'Publishing SOURCE ONLY: Rust compilation and all headset tests remain unverified.'
    }
    # Explicit paths only. Never "git add ." or a mirror push.
    $Stage = @('crates/rc-engine/Cargo.toml','crates/rc-engine/src/main.rs','crates/rc-engine/src/vr',
        'scripts/vr','docs/vr','.gitignore','.github/workflows')
    if (Test-Path '.github/upstream-workflows') { $Stage += '.github/upstream-workflows' }
    if (Test-Path 'Cargo.lock') { $Stage += 'Cargo.lock' }
    Invoke-Checked 'git' (@('add','-A','--') + $Stage)
    $Danger = @(& git diff --cached --name-only) | Where-Object { $_ -match '(?i)(\.iso$|\.cso$|(^|/)extracted/|(^|/)\.env($|\.)|(^|/)work/|(^|/)vr-captures/)' }
    if ($Danger) { throw "Refusing to commit local data: $($Danger -join ', ')" }
    # Preserve a user's configured identity; otherwise use a clearly labelled local automation identity.
    $Identity = & git config user.name
    $Email = & git config user.email
    $CommitArgs = @()
    if (-not $Identity) { $CommitArgs += @('-c','user.name=ReRAC VR PoC preparation') }
    if (-not $Email) { $CommitArgs += @('-c','user.email=rerac-vr-poc@localhost') }
    Invoke-Checked 'git' ($CommitArgs + @('commit','-m','Add experimental first-person VR terrain PoC; headset validation pending'))
    if ($PrepareOnly) {
        Write-Host "Prepared local source at $Destination. Nothing was published."
    } else {
        & '.\scripts\vr\Publish-Private.ps1' -Owner $Owner -RepositoryName $RepositoryName
    }
} finally { Pop-Location }
