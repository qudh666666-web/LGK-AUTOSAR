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
    [string]$ExecutablePath,

    [Parameter()]
    [switch]$ValidateOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Match the stable Windows behavior of the established Vector bridge: JSON,
# Chinese paths, DaVinci diagnostics and redirected output all use UTF-8.
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[Console]::InputEncoding = $utf8NoBom
[Console]::OutputEncoding = $utf8NoBom
$OutputEncoding = $utf8NoBom

# Avoid relying on $PSScriptRoot: it is empty in some Windows PowerShell
# launch paths even when the script itself was supplied with -File.
$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($scriptDirectory)) {
    throw 'Cannot determine the LGK-AUTOSAR script location; pass -ExecutablePath explicitly.'
}
if ([string]::IsNullOrWhiteSpace($ExecutablePath)) {
    # Release skills keep the wrapper beside the matching EXE.  The source
    # tree keeps it under scripts/, so retain that layout as a fallback.
    $runtimeExecutable = Join-Path $scriptDirectory 'lgk-autosar.exe'
    $rootExecutable = Join-Path $scriptDirectory '..\lgk-autosar.exe'
    if (Test-Path -LiteralPath $runtimeExecutable -PathType Leaf) {
        $ExecutablePath = $runtimeExecutable
    } elseif (Test-Path -LiteralPath $rootExecutable -PathType Leaf) {
        $ExecutablePath = $rootExecutable
    } else {
        $ExecutablePath = Join-Path $scriptDirectory '..\target\release\lgk-autosar.exe'
    }
}

# 先把用户输入统一成绝对路径，后续 Rust 端也会再次校验。
$project = (Resolve-Path -LiteralPath $ProjectPath -ErrorAction Stop).Path
$executable = (Resolve-Path -LiteralPath $ExecutablePath -ErrorAction Stop).Path
if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
    throw "Bridge executable was not found: $ExecutablePath"
}
$executableDirectory = Split-Path -Parent $executable
$hostExecutable = Join-Path $executableDirectory 'lgk-autosar-host.exe'

function ConvertTo-WindowsArgument {
    # MSVCRT argv 规则：参数含空白或引号时整体加引号；引号前的反斜杠翻倍并
    # 转义引号，结尾反斜杠翻倍，其余反斜杠保持原样。本包装器只传标志和路径。
    param([Parameter(Mandatory)][string]$Value)

    if ($Value -ne '' -and $Value -notmatch '[\s"]') {
        return $Value
    }
    $escaped = ''
    $backslashes = 0
    foreach ($character in $Value.ToCharArray()) {
        if ($character -eq '\') {
            $backslashes++
            continue
        }
        $prefix = '\' * $backslashes
        if ($character -eq '"') {
            $prefix = '\' * ($backslashes * 2 + 1)
        }
        $escaped += $prefix + $character
        $backslashes = 0
    }
    $escaped += '\' * ($backslashes * 2)
    return '"' + $escaped + '"'
}

function Invoke-BridgeExecutable {
    # 以硬超时执行一次 CLI 调用：任何一次外部调用都不允许无限挂起。超时先
    # 终止子进程树并抛错（附已捕获的部分输出）；正常结束返回退出码和标准
    # 输出/错误文本，由调用方沿用原有错误消息格式。工作目录显式取当前
    # PowerShell 位置：.NET Process.Start 不跟随 Push-Location，而 CLI 依赖
    # 工作目录中的 lgk-autosar.json。
    param(
        [Parameter(Mandatory)][string]$FilePath,
        [Parameter(Mandatory)][string[]]$Arguments,
        [Parameter(Mandatory)][int]$TimeoutSeconds
    )

    $processInfo = New-Object System.Diagnostics.ProcessStartInfo
    $processInfo.FileName = $FilePath
    $processInfo.Arguments = (($Arguments | ForEach-Object { ConvertTo-WindowsArgument $_ }) -join ' ')
    $processInfo.WorkingDirectory = (Get-Location).ProviderPath
    $processInfo.UseShellExecute = $false
    $processInfo.RedirectStandardOutput = $true
    $processInfo.RedirectStandardError = $true
    $processInfo.CreateNoWindow = $true
    $processInfo.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $processInfo.StandardErrorEncoding = [System.Text.Encoding]::UTF8

    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $processInfo
    if (-not $process.Start()) {
        throw "Failed to start the LGK-AUTOSAR executable: $FilePath"
    }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
        $timedOutProcessId = $process.Id
        & taskkill.exe /PID $timedOutProcessId /T /F | Out-Null
        $process.WaitForExit()
        $global:LASTEXITCODE = 1
        $partialStdout = ''
        $partialStderr = ''
        if ($stdoutTask.Wait(5000)) { $partialStdout = [string]$stdoutTask.Result }
        if ($stderrTask.Wait(5000)) { $partialStderr = [string]$stderrTask.Result }
        throw ("LGK-AUTOSAR executable '{0}' exceeded the {1}s timeout and its process tree was terminated. Partial stdout: {2} Partial stderr: {3}" -f $FilePath, $TimeoutSeconds, $partialStdout, $partialStderr)
    }
    $stdoutTask.Wait()
    $stderrTask.Wait()
    # 与原生命令保持同一契约：调用方（如 Initialize 脚本）依赖 $LASTEXITCODE
    # 反映最后一次 CLI 调用的退出码。.NET Process 不会自动设置它。
    $global:LASTEXITCODE = $process.ExitCode
    [pscustomobject]@{
        ExitCode = $process.ExitCode
        StdOut = [string]$stdoutTask.Result
        StdErr = [string]$stderrTask.Result
    }
}

function Get-BridgeIdentity([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Bridge executable was not found: $Path"
    }
    $versionResult = Invoke-BridgeExecutable -FilePath $Path -Arguments @('--version') -TimeoutSeconds 15
    $output = @($versionResult.StdOut -split "\r?\n" | Where-Object { $_ -ne '' })
    if ($versionResult.ExitCode -ne 0 -or $output.Count -ne 1) {
        throw "Bridge executable cannot complete the required --version check: $Path. The CLI/Host pair is stale or incomplete; rebuild both with 'cargo build --release' (or install one matching GitHub release package) before using this wrapper."
    }
    $match = [regex]::Match(
        [string]$output[0],
        '^(?<name>lgk-autosar(?:-host)?) (?<version>\d+\.\d+\.\d+) protocol=(?<protocol>\d+) build=(?<build>dev|[0-9a-f]{7,64})$'
    )
    if (-not $match.Success) {
        throw "Unexpected bridge version output from ${Path}: $($output -join ' ')"
    }
    [pscustomobject]@{
        Version = $match.Groups['version'].Value
        Protocol = $match.Groups['protocol'].Value
        Build = $match.Groups['build'].Value
    }
}

$cliIdentity = Get-BridgeIdentity -Path $executable
$hostIdentity = Get-BridgeIdentity -Path $hostExecutable
if ($cliIdentity.Version -ne $hostIdentity.Version -or
    $cliIdentity.Protocol -ne $hostIdentity.Protocol -or
    $cliIdentity.Build -ne $hostIdentity.Build) {
    throw "Bridge executable identities do not match: CLI=$($cliIdentity.Version)/p$($cliIdentity.Protocol)/$($cliIdentity.Build), Host=$($hostIdentity.Version)/p$($hostIdentity.Protocol)/$($hostIdentity.Build)"
}

$pairManifestPath = Join-Path $executableDirectory 'lgk-autosar-pair.json'
if (Test-Path -LiteralPath $pairManifestPath -PathType Leaf) {
    $pairManifest = [System.IO.File]::ReadAllText($pairManifestPath) | ConvertFrom-Json -ErrorAction Stop
    $actualCliHash = (Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash
    $actualHostHash = (Get-FileHash -LiteralPath $hostExecutable -Algorithm SHA256).Hash
    if ($actualCliHash -ine [string]$pairManifest.cli_sha256 -or
        $actualHostHash -ine [string]$pairManifest.host_sha256) {
        throw 'Bridge executable integrity does not match lgk-autosar-pair.json; reinstall one complete release package instead of mixing binaries'
    }
}

function Start-BridgeHost {
    # 由 Rust CLI 完成 Token、协议、版本和端口身份探测，避免只凭 TCP
    # 端口可连接就误判为当前 LGK-AUTOSAR Host。CLI 通过 bInheritHandles=FALSE
    # 的 CreateProcessW 拉起常驻 Host，调用方的管道/控制台句柄不会进入 Host。
    $hostResult = Invoke-BridgeExecutable -FilePath $executable -Arguments @('--start-host') -TimeoutSeconds 60
    if ($hostResult.ExitCode -ne 0) {
        $hostDetail = ("{0} {1}" -f $hostResult.StdOut, $hostResult.StdErr).Trim()
        throw "Bridge host failed to start or pass its protocol probe: $hostDetail"
    }
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
    # Keep in sync with CommandDispatcher::validate_one in
    # src/daemon/commands.rs. Rust remains authoritative for request behavior;
    # this list exists only to reject misspelled functions before Host startup.
    $allowedFunctions = @(
        'inspect_ecuc_containers',
        'inspect_autosar_model',
        'trace_autosar_model',
        'diff_ecuc',
        'set_ecuc_value',
        'set_ecuc_values',
        'find_module',
        'find_bsw_module',
        'find_module_template',
        'get_bsw_module_template',
        'get_param_definition',
        'get_bsw_param_definition',
        'locate_container',
        'verify_delivery',
        'edit_file',
        'get_errors_list',
        'auto_solve_errors',
        'generate_code',
        'update_project',
        'import_dbc',
        'shutdown_host'
    )
    $requestItems = @($requestObject)
    if ($requestItems.Count -eq 0) {
        throw 'Request must contain at least one LGK-AUTOSAR function call'
    }
    foreach ($item in $requestItems) {
        if (($null -eq $item) -or ($item.PSObject.Properties.Name -notcontains 'func')) {
            throw "Every request item must contain a 'func' field"
        }
        if ($allowedFunctions -notcontains [string]$item.func) {
            throw "Unsupported LGK-AUTOSAR function: $($item.func)"
        }
    }
    $mutatingFunctions = @(
        'set_ecuc_value', 'set_ecuc_values', 'edit_file', 'auto_solve_errors', 'generate_code',
        'update_project', 'import_dbc', 'shutdown_host'
    )
    if ($requestObject -is [System.Array] -and $requestObject.Count -gt 1) {
        foreach ($item in $requestItems) {
            if ($mutatingFunctions -contains [string]$item.func) {
                throw "Mutating function '$($item.func)' must be sent as a standalone request, not inside a multi-item array"
            }
        }
    }

    if ($ValidateOnly) {
        Push-Location -LiteralPath $project
        try {
            $doctorResult = Invoke-BridgeExecutable -FilePath $executable -Arguments @('--doctor', '--request-file', $requestPath) -TimeoutSeconds 90
            if ($doctorResult.ExitCode -ne 0) {
                $doctorDetail = ("{0} {1}" -f $doctorResult.StdOut, $doctorResult.StdErr).Trim()
                throw "LGK-AUTOSAR doctor failed: $doctorDetail"
            }
            $doctorResult.StdOut | Write-Output
        } finally {
            Pop-Location
        }
        return
    }

    # shutdown 是独立控制命令：不应为了关闭而新启动一个 Host。
    $isShutdown = ($requestObject -isnot [System.Array]) -and ($requestObject.func -eq 'shutdown_host')

    Push-Location -LiteralPath $project
    try {
        if (-not $isShutdown) {
            Start-BridgeHost
        }
        # 真正的请求处理从这里进入 Rust CLI，再转给 resident Host。
        $requestResult = Invoke-BridgeExecutable -FilePath $executable -Arguments @('--request-file', $requestPath) -TimeoutSeconds 280
        if ($requestResult.ExitCode -ne 0) {
            $requestDetail = ("{0} {1}" -f $requestResult.StdOut, $requestResult.StdErr).Trim()
            throw "Bridge request failed (exit $($requestResult.ExitCode), request: $requestPath): $requestDetail"
        }
        $requestResult.StdOut | Write-Output
    } finally {
        Pop-Location
    }
} finally {
    # 只删除本次包装器创建的临时请求，不触碰工程文件。
    if ($null -ne $temporaryRequest -and (Test-Path -LiteralPath $temporaryRequest)) {
        Remove-Item -LiteralPath $temporaryRequest -Force
    }
}
