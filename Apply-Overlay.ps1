[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Repository)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Root = (Resolve-Path -LiteralPath $Repository).Path
$Manifest = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'patch-manifest.json') -Raw | ConvertFrom-Json
$Utf8 = New-Object System.Text.UTF8Encoding($false)
Push-Location $Root
try {
    $Head = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $Head -ne $Manifest.upstreamCommit) { throw 'The checkout is not the pinned upstream commit. No files changed.' }
    $Dirty = & git status --porcelain
    if ($LASTEXITCODE -ne 0 -or $Dirty) { throw 'Apply only to a clean, fresh checkout. No files changed.' }
    $Changes = @()
    foreach ($Patch in $Manifest.patches) {
        if ($Patch.path -match '(^|/)\.\.(/|$)' -or [IO.Path]::IsPathRooted($Patch.path)) { throw 'Unsafe patch path.' }
        $Path = Join-Path $Root $Patch.path
        $Blob = (& git hash-object -- $Patch.path).Trim()
        if ($LASTEXITCODE -ne 0 -or $Blob -ne $Patch.expectedBlobSha) { throw "Upstream blob mismatch: $($Patch.path). No files changed." }
        $Text = [IO.File]::ReadAllText($Path).Replace("`r`n", "`n")
        foreach ($Change in $Patch.replacements) {
            $Count = [regex]::Matches($Text, [regex]::Escape([string]$Change.old)).Count
            if ($Count -ne 1) { throw "Expected one patch anchor in $($Patch.path), found $Count. No files changed." }
            $Text = $Text.Replace([string]$Change.old, [string]$Change.new)
        }
        $Changes += [PSCustomObject]@{ Path=$Path; Text=$Text }
    }
    $LockBlob = (& git hash-object -- Cargo.lock).Trim()
    if ($LASTEXITCODE -ne 0 -or $LockBlob -ne $Manifest.lockfile.expectedBlobSha) { throw 'Upstream lockfile mismatch. No files changed.' }
    $LockText = [IO.File]::ReadAllText((Join-Path $PSScriptRoot $Manifest.lockfile.source))
    $Changes += [PSCustomObject]@{ Path=(Join-Path $Root 'Cargo.lock'); Text=$LockText }
    $PayloadRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'overlay')).Path
    $Payload = @(Get-ChildItem -LiteralPath $PayloadRoot -File -Recurse -Force)
    foreach ($File in $Payload) {
        $Relative = $File.FullName.Substring($PayloadRoot.Length).TrimStart([char[]]'\/')
        if (Test-Path -LiteralPath (Join-Path $Root $Relative)) { throw "Refusing to overwrite payload path $Relative. No files changed." }
    }
    # All guards passed. Apply the small upstream changes and add new files.
    foreach ($Change in $Changes) { [IO.File]::WriteAllText($Change.Path, $Change.Text, $Utf8) }
    foreach ($File in $Payload) {
        $Relative = $File.FullName.Substring($PayloadRoot.Length).TrimStart([char[]]'\/')
        $Target = Join-Path $Root $Relative
        [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($Target)) | Out-Null
        [IO.File]::Copy($File.FullName, $Target, $false)
    }
    $IgnorePath = Join-Path $Root '.gitignore'
    $Ignore = [IO.File]::ReadAllText($IgnorePath)
    $Ignore += "`n# VR PoC local-only data; never stage game assets, captures or credentials.`n*.iso`n*.ISO`n*.cso`n*.CSO`nextracted/`nwork/vr/`nvr-captures/`n.env`n.env.*`n"
    [IO.File]::WriteAllText($IgnorePath, $Ignore, $Utf8)
    # Do not inherit an upstream release/publishing workflow in a private experiment.
    $WorkflowDir = Join-Path $Root '.github/workflows'
    $Archive = Join-Path $Root '.github/upstream-workflows'
    foreach ($File in @(Get-ChildItem -LiteralPath $WorkflowDir -File -ErrorAction SilentlyContinue)) {
        if ($File.Name -ne 'vr-poc.yml' -and $File.Extension -in @('.yml','.yaml')) {
            [IO.Directory]::CreateDirectory($Archive) | Out-Null
            Move-Item -LiteralPath $File.FullName -Destination (Join-Path $Archive ($File.Name + '.disabled'))
        }
    }
    Write-Host 'Overlay applied. This does not establish compilation or headset compatibility.'
} finally { Pop-Location }
