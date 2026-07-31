[CmdletBinding(DefaultParameterSetName = 'Inline')]
param(
    [Parameter()]
    [string]$ProjectPath = (Get-Location).Path,

    [Parameter(Mandatory, ParameterSetName = 'Inline')]
    [string]$Request,

    [Parameter(Mandatory, ParameterSetName = 'File')]
    [string]$RequestFile,

    [Parameter()]
    [string]$ExecutablePath = (Join-Path $PSScriptRoot '..\autosar-ecuc-bridge.exe')
)

$project = (Resolve-Path -LiteralPath $ProjectPath -ErrorAction Stop).Path
$executable = (Resolve-Path -LiteralPath $ExecutablePath -ErrorAction Stop).Path
if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
    throw "Bridge executable was not found: $ExecutablePath"
}

$temporaryRequest = $null
try {
    if ($PSCmdlet.ParameterSetName -eq 'Inline') {
        $null = $Request | ConvertFrom-Json -ErrorAction Stop
        $temporaryRequest = [System.IO.Path]::GetTempFileName()
        [System.IO.File]::WriteAllText($temporaryRequest, $Request, [System.Text.UTF8Encoding]::new($false))
        $requestPath = $temporaryRequest
    } else {
        $requestPath = (Resolve-Path -LiteralPath $RequestFile -ErrorAction Stop).Path
    }

    Push-Location -LiteralPath $project
    try {
        & $executable --request-file $requestPath
        if ($LASTEXITCODE -ne 0) {
            throw "Bridge request failed with exit code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }
} finally {
    if ($null -ne $temporaryRequest -and (Test-Path -LiteralPath $temporaryRequest)) {
        Remove-Item -LiteralPath $temporaryRequest -Force
    }
}
