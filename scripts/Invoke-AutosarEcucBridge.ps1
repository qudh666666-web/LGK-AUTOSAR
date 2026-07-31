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
$executableDirectory = Split-Path -Parent $executable
$hostExecutable = Join-Path $executableDirectory 'autosar-ecuc-bridge-host.exe'
$hostPort = 32483

function Test-BridgeHost {
    $client = [System.Net.Sockets.TcpClient]::new()
    try {
        $task = $client.ConnectAsync('127.0.0.1', $hostPort)
        return $task.Wait(250) -and $client.Connected
    } catch {
        return $false
    } finally {
        $client.Dispose()
    }
}

function Start-BridgeHost {
    if (Test-BridgeHost) {
        return
    }
    if (-not (Test-Path -LiteralPath $hostExecutable -PathType Leaf)) {
        throw "Bridge host executable was not found: $hostExecutable"
    }

    $tokenDirectory = Join-Path $executableDirectory '.autosar-ecuc-bridge'
    $tokenPath = Join-Path $tokenDirectory 'host.token'
    if (-not (Test-Path -LiteralPath $tokenPath -PathType Leaf)) {
        New-Item -ItemType Directory -Force -Path $tokenDirectory | Out-Null
        $bytes = New-Object byte[] 32
        $generator = [System.Security.Cryptography.RandomNumberGenerator]::Create()
        try {
            $generator.GetBytes($bytes)
        } finally {
            $generator.Dispose()
        }
        $token = -join ($bytes | ForEach-Object { $_.ToString('x2') })
        [System.IO.File]::WriteAllText($tokenPath, $token, [System.Text.UTF8Encoding]::new($false))
    }

    $quotedTokenPath = '"' + $tokenPath + '"'
    Start-Process -FilePath $hostExecutable `
        -ArgumentList @('--port', $hostPort, '--token-file', $quotedTokenPath) `
        -WindowStyle Hidden | Out-Null
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    while (-not (Test-BridgeHost)) {
        if ([DateTime]::UtcNow -ge $deadline) {
            throw 'Bridge host did not become ready within 15 seconds'
        }
        Start-Sleep -Milliseconds 100
    }
    Start-Sleep -Seconds 1
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

    $requestObject = [System.IO.File]::ReadAllText($requestPath) | ConvertFrom-Json -ErrorAction Stop
    $isShutdown = ($requestObject -isnot [System.Array]) -and ($requestObject.func -eq 'shutdown_host')

    Push-Location -LiteralPath $project
    try {
        if (-not $isShutdown) {
            Start-BridgeHost
        }
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
