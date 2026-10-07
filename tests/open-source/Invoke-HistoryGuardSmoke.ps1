[CmdletBinding()]
param(
    [string]$RepositoryRoot
)
$ErrorActionPreference = 'Stop'
if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    $RepositoryRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
}
$repository = (Resolve-Path -LiteralPath $RepositoryRoot).ProviderPath
$guard = Join-Path $repository 'tests\open-source\Invoke-OpenSourceGuard.ps1'
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('lgk-history-guard-' + [guid]::NewGuid().ToString('N'))
$utf8 = New-Object Text.UTF8Encoding($false)
$privateEmail = '100000' + '@' + 'qq.com'
$oldBrand = 'gyx' + '-vector'
$checks = 0

function Invoke-FixtureGit([string[]]$Arguments) {
    # Windows PowerShell treats even successful Git stderr as an error record.
    # Capture it, then decide success from Git's exit code.
    $ErrorActionPreference = 'Continue'
    $output = @(& git -C $fixture @Arguments 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "Fixture Git failed: $($output -join ' ')" }
    return ,$output
}
function Assert-GuardPass {
    $result = (& $guard -RepositoryRoot $fixture -IncludeHistory | Out-String) | ConvertFrom-Json
    if (-not $result.valid -or -not $result.history_checked -or $result.legacy_author_commits -ne 0) {
        throw 'Unexpected public history guard result'
    }
    $script:checks++
}
function Assert-GuardFailure([string]$Pattern) {
    $failure = $null
    try { & $guard -RepositoryRoot $fixture -IncludeHistory | Out-Null } catch { $failure = $_.Exception.Message }
    if ($null -eq $failure -or $failure -notmatch $Pattern) { throw "Expected guard failure: $Pattern" }
    $script:checks++
}
try {
    New-Item -ItemType Directory -Path $fixture | Out-Null
    Invoke-FixtureGit -Arguments @('init') | Out-Null
    Invoke-FixtureGit -Arguments @('config', 'user.name', 'LGK History Test') | Out-Null
    Invoke-FixtureGit -Arguments @('config', 'user.email', 'history-test@users.noreply.github.com') | Out-Null
    [IO.File]::WriteAllText((Join-Path $fixture 'README.md'), 'Synthetic public source', $utf8)
    Invoke-FixtureGit -Arguments @('add', 'README.md') | Out-Null
    Invoke-FixtureGit -Arguments @('commit', '-m', 'Public baseline') | Out-Null
    $baseline = (Invoke-FixtureGit -Arguments @('rev-parse', 'HEAD'))[0]
    Assert-GuardPass

    Invoke-FixtureGit -Arguments @('-c', ('user.email=' + $privateEmail), 'commit', '--allow-empty', '-m', 'Private author example') | Out-Null
    $privateCommit = (Invoke-FixtureGit -Arguments @('rev-parse', 'HEAD'))[0]
    Assert-GuardFailure 'personal QQ author emails=1'
    # An empty commit with identical source is still rejected: checking only
    # current content must not bypass the history gate.
    Invoke-FixtureGit -Arguments @('diff', '--exit-code', $baseline, $privateCommit) | Out-Null
    $script:checks++
    Invoke-FixtureGit -Arguments @('branch', 'private-sibling', $privateCommit) | Out-Null
    Invoke-FixtureGit -Arguments @('checkout', '--detach', $baseline) | Out-Null
    Assert-GuardPass

    # A removed private value remains forbidden in newly introduced history.
    [IO.File]::WriteAllText((Join-Path $fixture 'README.md'), $oldBrand, $utf8)
    Invoke-FixtureGit -Arguments @('add', 'README.md') | Out-Null
    Invoke-FixtureGit -Arguments @('commit', '-m', 'Synthetic historical product value') | Out-Null
    [IO.File]::WriteAllText((Join-Path $fixture 'README.md'), 'Synthetic public source', $utf8)
    Invoke-FixtureGit -Arguments @('add', 'README.md') | Out-Null
    Invoke-FixtureGit -Arguments @('commit', '-m', 'Remove historical product value') | Out-Null
    Assert-GuardFailure 'old-brand commits='
    Invoke-FixtureGit -Arguments @('checkout', '--detach', $baseline) | Out-Null
    Assert-GuardPass
    [pscustomobject]@{valid=$true;assertions=$checks} | ConvertTo-Json
} finally {
    if (Test-Path -LiteralPath $fixture) {
        $resolved = (Resolve-Path -LiteralPath $fixture).ProviderPath
        $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
        if (-not $resolved.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -or
            (Split-Path -Leaf $resolved) -notlike 'lgk-history-guard-*') { throw 'Unexpected history fixture cleanup path' }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
