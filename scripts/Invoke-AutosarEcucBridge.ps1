# 人和自动化工具的统一入口：负责准备请求和常驻 Host，
# 不直接解析 ECUC，也不直接启动 DaVinci 生成器。
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

# 先把用户输入统一成绝对路径，后续 Rust 端也会再次校验。
$project = (Resolve-Path -LiteralPath $ProjectPath -ErrorAction Stop).Path
$executable = (Resolve-Path -LiteralPath $ExecutablePath -ErrorAction Stop).Path
if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
    throw "Bridge executable was not found: $ExecutablePath"
}
$executableDirectory = Split-Path -Parent $executable
$hostExecutable = Join-Path $executableDirectory 'autosar-ecuc-bridge-host.exe'
$hostPort = 32483

function Test-BridgeHost {
    # Host 只监听本机回环地址；这里的短连接仅用于探测是否已启动。
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

    # Token 不是许可证。它只防止本机其他进程随意调用 Host 端口。
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

    # Host 与请求程序必须使用同一份 Token；Host 是独立后台进程。
    $quotedTokenPath = '"' + $tokenPath + '"'
    Start-Process -FilePath $hostExecutable `
        -ArgumentList @('--port', $hostPort, '--token-file', $quotedTokenPath) `
        -WindowStyle Hidden | Out-Null
    # 不假设 Start-Process 返回时 Host 已完成监听，最多等 15 秒。
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
        # 长 JSON 经过命令行转义容易损坏，统一改为 UTF-8 临时文件传给 Rust。
        $temporaryRequest = [System.IO.Path]::GetTempFileName()
        [System.IO.File]::WriteAllText($temporaryRequest, $Request, [System.Text.UTF8Encoding]::new($false))
        $requestPath = $temporaryRequest
    } else {
        $requestPath = (Resolve-Path -LiteralPath $RequestFile -ErrorAction Stop).Path
    }

    $requestObject = [System.IO.File]::ReadAllText($requestPath) | ConvertFrom-Json -ErrorAction Stop
    # shutdown 是独立控制命令：不应为了关闭而新启动一个 Host。
    $isShutdown = ($requestObject -isnot [System.Array]) -and ($requestObject.func -eq 'shutdown_host')

    Push-Location -LiteralPath $project
    try {
        if (-not $isShutdown) {
            Start-BridgeHost
        }
        # 真正的请求处理从这里进入 Rust CLI，再转给 resident Host。
        & $executable --request-file $requestPath
        if ($LASTEXITCODE -ne 0) {
            throw "Bridge request failed with exit code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }
} finally {
    # 只删除本次包装器创建的临时请求，不触碰工程文件。
    if ($null -ne $temporaryRequest -and (Test-Path -LiteralPath $temporaryRequest)) {
        Remove-Item -LiteralPath $temporaryRequest -Force
    }
}
