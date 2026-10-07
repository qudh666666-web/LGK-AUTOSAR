# Maintainer-only end-to-end check. Vendor input and all logs stay outside Git.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SampleProjectPath,
    [Parameter(Mandatory)][string]$ToolPath,
    [Parameter(Mandatory)][string]$WorkRoot
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$utf8=New-Object System.Text.UTF8Encoding($false)
$central=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).ProviderPath
& (Join-Path $PSScriptRoot 'Invoke-LGKNativeBooleanProbe.ps1') -SampleProjectPath $SampleProjectPath -ToolPath $ToolPath -WorkRoot $WorkRoot -PrepareOnly | Out-Null
$root=(Resolve-Path -LiteralPath $WorkRoot).ProviderPath
$sip=(Resolve-Path -LiteralPath $ToolPath).ProviderPath
$sample=(Resolve-Path -LiteralPath $SampleProjectPath).ProviderPath
$dpa=@(Get-ChildItem -LiteralPath $root -Filter '*.dpa' -File)[0].FullName
$dpaXml=[xml][IO.File]::ReadAllText($dpa)
$registered=@($dpaXml.ProjectAssistant.Folders.ApplicationComponentFolders.ApplicationComponentFolder)
if ($registered.Count -ne 1) {throw 'Acceptance sample must have one application component input folder.'}
$aswFolder=[IO.Path]::GetFullPath((Join-Path $root ([string]$registered[0])))
if (-not $aswFolder.StartsWith($root+'\',[StringComparison]::OrdinalIgnoreCase)) {throw 'ASW input folder escapes disposable root.'}
$aswFile=Join-Path $aswFolder 'LGKAswAcceptance.arxml'
$nativeDir=Join-Path $root 'native-scripts'
Copy-Item -LiteralPath (Join-Path $central 'assets\experiments\LGKAswAcceptance.dvgroovy') -Destination $nativeDir
$baseline=@{}
foreach ($file in Get-ChildItem -LiteralPath $sample -Recurse -File -Force) {
    $baseline[$file.FullName.Substring($sample.TrimEnd('\').Length+1)]=(Get-FileHash -LiteralPath $file.FullName).Hash
}
$ini=Join-Path $sip 'DaVinciConfigurator\Core\DVCfgCmd.ini'
$iniHash=(Get-FileHash -LiteralPath $ini).Hash
$wrapper=Join-Path $PSScriptRoot 'Invoke-LGKAutosar.ps1'
& (Join-Path $PSScriptRoot 'Initialize-LGKAutosarProject.ps1') -ProjectPath $root -ToolPath $sip | Out-Null
$bundle=@{
    package='LGKAswAcceptance'
    types=@(
        @{kind='alias';name='Counter';type_ref='/AUTOSAR_Platform/ImplementationDataTypes/uint8'},
        @{kind='scalar';name='NativeByte';base_type_ref='/AUTOSAR_Platform/BaseTypes/uint8'},
        @{kind='array';name='Buffer';type_ref='/LGKAswAcceptance/Counter';length=4},
        @{kind='record';name='Pair';fields=@(@{name='Count';type_ref='/LGKAswAcceptance/Counter'},@{name='Bytes';type_ref='/LGKAswAcceptance/Buffer'})}
    )
    interfaces=@(
        @{kind='sender_receiver';name='Values';data_elements=@(@{name='Count';type_ref='/LGKAswAcceptance/Counter'})},
        @{kind='client_server';name='Api';operations=@(@{name='Get';arguments=@(@{name='Result';type_ref='/LGKAswAcceptance/Counter';direction='OUT'})})}
    )
    components=@(
        @{name='Producer';ports=@(@{name='Out';direction='provide';interface_ref='/LGKAswAcceptance/Values'});runnables=@(@{name='Tick';symbol='LGKProducer_Tick';period_seconds=0.01;writes=@(@{port='Out';data_element='Count'})})},
        @{name='Consumer';ports=@(@{name='In';direction='require';interface_ref='/LGKAswAcceptance/Values'});runnables=@(@{name='Tick';symbol='LGKConsumer_Tick';reads=@(@{port='In';data_element='Count'})})}
    )
    compositions=@(@{name='Root';instances=@(@{name='Tx';type_ref='/LGKAswAcceptance/Producer'},@{name='Rx';type_ref='/LGKAswAcceptance/Consumer'});
        ports=@(@{name='Out';direction='provide';interface_ref='/LGKAswAcceptance/Values'});
        connections=@(@{name='Link';provider=@{instance='Tx';port='Out'};requester=@{instance='Rx';port='In'}});
        delegations=@(@{name='Expose';inner=@{instance='Tx';port='Out'};outer_port='Out'})})
}
function Invoke-NativeStage([string]$Stage,[bool]$Deleted,[double]$Period) {
    $manifest=@{schema=1;disposable=$true;phase='prepared';root=$root;project_file=$dpa;stage=$Stage;deleted=$Deleted;period=$Period}
    [IO.File]::WriteAllText((Join-Path $root 'lgk-asw-acceptance.json'),($manifest | ConvertTo-Json),$utf8)
    $stdout=Join-Path $root "$Stage.stdout.log"
    $stderr=Join-Path $root "$Stage.stderr.log"
    $command=Join-Path $sip 'DaVinciConfigurator\Core\DVCfgCmd.exe'
    $arguments=@('--launcher.ini',('"'+(Join-Path $root 'DVCfgCmd.probe.ini')+'"'),'--project',('"'+$dpa+'"'),
        '--scriptLocations',('"'+$nativeDir+'"'),'--scriptTask','LGKAswAcceptance','--ignoreUserScriptLocations','--verbose','ERROR')
    $process=Start-Process -FilePath $command -ArgumentList $arguments -WorkingDirectory $root -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
    $watch=[Diagnostics.Stopwatch]::StartNew()
    $peak=0L
    try {
        while (-not $process.WaitForExit(1000)) {
            if ($watch.Elapsed.TotalSeconds -gt 120) {throw 'ASW native stage exceeded 120 seconds.'}
            $pending=New-Object 'System.Collections.Generic.Queue[int]'
            $seen=New-Object 'System.Collections.Generic.HashSet[int]'
            $pending.Enqueue($process.Id)
            while ($pending.Count -gt 0) {
                $ownedPid=$pending.Dequeue()
                if (-not $seen.Add($ownedPid)) {continue}
                $running=Get-Process -Id $ownedPid -ErrorAction SilentlyContinue
                if ($null -ne $running) {
                    $peak=[Math]::Max($peak,$running.PrivateMemorySize64)
                    if ($running.PrivateMemorySize64 -ge 1800MB) {throw 'Owned DaVinci process approaches 2 GiB.'}
                }
                foreach ($child in @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$ownedPid")) {$pending.Enqueue([int]$child.ProcessId)}
            }
        }
        $process.WaitForExit()
        if ($process.ExitCode -ne 0) {throw "Native ASW stage failed, exit=$($process.ExitCode); see $stdout"}
        $lines=@(Get-Content -LiteralPath $stdout | Where-Object {$_ -match 'LGK_ASW_ACCEPTANCE_JSON:'})
        if ($lines.Count -ne 1) {throw 'Missing unique native ASW evidence record.'}
        $record=($lines[0] -split 'LGK_ASW_ACCEPTANCE_JSON:',2)[1] | ConvertFrom-Json
        $record | Add-Member NoteProperty elapsed_seconds $watch.Elapsed.TotalSeconds
        $record | Add-Member NoteProperty peak_owned_private_bytes $peak
        [IO.File]::WriteAllText((Join-Path $root "$Stage.native-evidence.json"),($record | ConvertTo-Json),$utf8)
        return $record
    } finally {
        if (-not $process.HasExited) {& taskkill.exe /PID $process.Id /T /F | Out-Null}
        $process.Dispose()
    }
}
$records=@()
try {
    $request=@{func='write_asw_bundle';file=$aswFile;expected=$null;bundle=$bundle;preview=$true}
    & $wrapper -ProjectPath $root -Request ($request | ConvertTo-Json -Depth 15 -Compress) | Out-Null
    if (Test-Path -LiteralPath $aswFile) {throw 'Preview wrote the ASW file.'}
    $request.preview=$false
    & $wrapper -ProjectPath $root -Request ($request | ConvertTo-Json -Depth 15 -Compress) | Out-Null
    & $wrapper -ProjectPath $root -Request '{"func":"shutdown_host"}' | Out-Null
    $records+=Invoke-NativeStage 'created' $false 0.01
    $request.expected=[Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes($aswFile))
    $bundle.components[0].runnables[0].period_seconds=0.02
    & $wrapper -ProjectPath $root -Request ($request | ConvertTo-Json -Depth 15 -Compress) | Out-Null
    & $wrapper -ProjectPath $root -Request '{"func":"shutdown_host"}' | Out-Null
    $records+=Invoke-NativeStage 'updated' $false 0.02
    $delete=@{func='write_asw_bundle';file=$aswFile;expected=[Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes($aswFile));delete=$true}
    & $wrapper -ProjectPath $root -Request ($delete | ConvertTo-Json -Depth 4 -Compress) | Out-Null
    & $wrapper -ProjectPath $root -Request '{"func":"shutdown_host"}' | Out-Null
    $records+=Invoke-NativeStage 'deleted' $true 0
    foreach($relative in $baseline.Keys) {
        if ((Get-FileHash -LiteralPath (Join-Path $sample $relative)).Hash -ne $baseline[$relative]) {throw 'Input sample changed.'}
    }
    if ((Get-FileHash -LiteralPath $ini).Hash -ne $iniHash) {throw 'Original SIP launcher INI changed.'}
    $result=@{passed=$true;input_files_unchanged=$true;input_files_checked=$baseline.Count;vendor_ini_unchanged=$true;stages=$records;
        scope='saved ASW create/update/delete and native reload/export; no RTE generation or RTE-OS mapping acceptance'}
    [IO.File]::WriteAllText((Join-Path $root 'asw-evidence.json'),($result | ConvertTo-Json -Depth 6),$utf8)
    $result | ConvertTo-Json -Depth 6
} finally {
    & $wrapper -ProjectPath $root -Request '{"func":"shutdown_host"}' | Out-Null
}
