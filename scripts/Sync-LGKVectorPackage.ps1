[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$DestinationRoot,

    [Parameter()]
    [string]$SourceRoot = (Split-Path -Parent $PSScriptRoot),

    [Parameter()]
    [switch]$IncludeBinaries
)

$source = (Resolve-Path -LiteralPath $SourceRoot -ErrorAction Stop).Path
if (-not (Test-Path -LiteralPath (Join-Path $source 'Cargo.toml') -PathType Leaf) -or
    -not (Test-Path -LiteralPath (Join-Path $source 'SKILL.md') -PathType Leaf) -or
    -not (Test-Path -LiteralPath (Join-Path $source 'src') -PathType Container)) {
    throw "SourceRoot is not an LGK-Vector source tree: $source"
}

$destination = [System.IO.Path]::GetFullPath($DestinationRoot)
$root = [System.IO.Path]::GetPathRoot($destination)
if ([string]::IsNullOrWhiteSpace($destination) -or $destination -eq $root) {
    throw "DestinationRoot must be a dedicated LGK-Vector directory"
}
New-Item -ItemType Directory -Force -Path $destination | Out-Null

# 使用明确清单，避免把 .git、target、客户文件或本地 Token 带进工程包。
$files = @(
    '.gitignore'
    'Cargo.toml'
    'Cargo.lock'
    'LICENSE'
    'NOTICE'
    'README.md'
    'CHANGELOG.md'
    'SKILL.md'
    'agents\openai.yaml'
)
$directories = @('assets', 'docs', 'scripts', 'src', 'tests')

foreach ($relative in $files) {
    $sourceFile = Join-Path $source $relative
    if (-not (Test-Path -LiteralPath $sourceFile -PathType Leaf)) {
        throw "Required source file is missing: $sourceFile"
    }
    $destinationFile = Join-Path $destination $relative
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $destinationFile) | Out-Null
    Copy-Item -LiteralPath $sourceFile -Destination $destinationFile -Force
}

foreach ($relative in $directories) {
    $sourceDirectory = Join-Path $source $relative
    $destinationDirectory = Join-Path $destination $relative
    New-Item -ItemType Directory -Force -Path $destinationDirectory | Out-Null
    Copy-Item -Path (Join-Path $sourceDirectory '*') -Destination $destinationDirectory -Recurse -Force
}

if ($IncludeBinaries) {
    $release = Join-Path $source 'target\release'
    foreach ($name in @('lgk-vector.exe', 'lgk-vector-host.exe')) {
        $binary = Join-Path $release $name
        if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
            throw "Release binary is missing; run cargo build --release --locked first: $binary"
        }
        Copy-Item -LiteralPath $binary -Destination (Join-Path $destination $name) -Force
    }
}

[pscustomobject]@{
    source = $source
    destination = $destination
    binaries = [bool]$IncludeBinaries
}

