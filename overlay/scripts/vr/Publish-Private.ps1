[CmdletBinding()]
param([string]$Owner='npiriou',[string]$RepositoryName='rerac-vr-poc')
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
function Checked([string]$Program,[string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed (exit $LASTEXITCODE). Nothing will be force-pushed." }
}
$Root=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
Push-Location $Root
try {
    foreach ($Tool in @('git','gh')) { if (-not (Get-Command $Tool -ErrorAction SilentlyContinue)) {throw "Missing $Tool"} }
    if ($RepositoryName -notmatch '^[A-Za-z0-9][A-Za-z0-9_.-]{0,99}$') {throw 'Invalid repository name.'}
    $Login=(& gh api user --jq .login).Trim()
    if ($LASTEXITCODE -ne 0 -or $Login -ine $Owner) {throw 'Authenticate GitHub CLI to the intended account before publishing.'}
    $Dirty=& git status --porcelain
    if ($LASTEXITCODE -ne 0 -or $Dirty) {throw 'Commit the intended changes first. This script requires a clean worktree.'}
    $Branch=(& git branch --show-current).Trim()
    if ($Branch -ne 'main') {throw 'Expected local main branch.'}
    $Remotes=@(& git remote)
    if ($Remotes -contains 'origin') {throw 'An origin remote already exists. Refusing to replace it or publish to an existing destination.'}
    $Upstream=(& git remote get-url upstream).Trim()
    if ($Upstream -ne 'https://github.com/re-rac/rerac.git') {throw 'Unexpected upstream remote.'}
    Checked 'git' @('merge-base','--is-ancestor','89c87e98f9d8296187df2a6310c67b32144090d3','HEAD')
    # Create an independent PRIVATE repository. API fails if that name already exists.
    $Created=(& gh api user/repos --method POST -f "name=$RepositoryName" -F 'private=true' -f 'description=Experimental ReRAC VR terrain PoC; hardware validation pending' --jq .full_name).Trim()
    if ($LASTEXITCODE -ne 0 -or $Created -ine "$Owner/$RepositoryName") {throw 'Private repository creation failed. No push attempted.'}
    $InfoText=& gh repo view "$Owner/$RepositoryName" --json visibility,isFork,nameWithOwner,url
    if ($LASTEXITCODE -ne 0) {throw 'Cannot verify repository privacy. No push attempted.'}
    $Info=$InfoText | ConvertFrom-Json
    if ($Info.visibility -ne 'PRIVATE' -or $Info.isFork -or $Info.nameWithOwner -ine "$Owner/$RepositoryName") {
        throw 'Destination is not the expected independent private repository. No push attempted.'
    }
    Checked 'git' @('remote','add','origin',"https://github.com/$Owner/$RepositoryName.git")
    # Use the local gh credential helper; do not expose or persist token strings.
    Checked 'git' @('-c','credential.helper=','-c','credential.helper=!gh auth git-credential','push','-u','origin','main')
    $Local=(& git rev-parse HEAD).Trim()
    $Remote=(& gh api "repos/$Owner/$RepositoryName/commits/main" --jq .sha).Trim()
    if ($LASTEXITCODE -ne 0 -or $Local -ne $Remote) {throw 'Push verification failed; compare local and remote HEAD before continuing.'}
    $Visibility=(& gh repo view "$Owner/$RepositoryName" --json visibility --jq .visibility).Trim()
    if ($LASTEXITCODE -ne 0 -or $Visibility -ne 'PRIVATE') {throw 'Post-push privacy verification failed. Inspect the repository settings immediately.'}
    Write-Host "Published and verified PRIVATE repository: $($Info.url)"
    Write-Host "Commit: $Local"
} finally {Pop-Location}
