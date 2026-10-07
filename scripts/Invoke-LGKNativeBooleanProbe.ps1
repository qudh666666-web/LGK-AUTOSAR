# Maintainer-only DaVinci 5 experiment; never edits the input sample.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SampleProjectPath,
    [Parameter(Mandatory)][string]$ToolPath,
    [Parameter(Mandatory)][string]$WorkRoot,
    [switch]$PrepareOnly
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$utf8 = New-Object System.Text.UTF8Encoding($false)
$source = (Resolve-Path -LiteralPath $SampleProjectPath).ProviderPath
$sip = (Resolve-Path -LiteralPath $ToolPath).ProviderPath
$workspace = [IO.Path]::GetFullPath($WorkRoot).TrimEnd('\')
$central = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).ProviderPath.TrimEnd('\')
foreach ($protected in @($source, $sip, $central)) {
    if ($workspace.Equals($protected, [StringComparison]::OrdinalIgnoreCase) -or
        $workspace.StartsWith($protected.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'WorkRoot must be outside the source, SIP and central repository.'
    }
}
if (Test-Path -LiteralPath $workspace) { throw 'WorkRoot already exists; preserve previous evidence.' }
foreach ($path in @($source,$sip)) {
    if ((Get-Item -LiteralPath $path -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {throw 'Source/SIP root cannot be a reparse point.'}
}
$ancestor=[IO.Path]::GetDirectoryName($workspace)
while (-not [string]::IsNullOrEmpty($ancestor)) {
    if ((Test-Path -LiteralPath $ancestor) -and
        (Get-Item -LiteralPath $ancestor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw 'WorkRoot ancestors cannot be reparse points.'
    }
    $ancestor=[IO.Path]::GetDirectoryName($ancestor)
}
if (@(Get-ChildItem -LiteralPath $source -Recurse -Force | Where-Object {$_.Attributes -band [IO.FileAttributes]::ReparsePoint}).Count -gt 0) {
    throw 'Sample contains reparse points; no copy or native execution was performed.'
}
$dpaFiles = @(Get-ChildItem -LiteralPath $source -File -Filter '*.dpa')
if ($dpaFiles.Count -ne 1) { throw 'Sample must have exactly one root DPA.' }
$command = Join-Path $sip 'DaVinciConfigurator\Core\DVCfgCmd.exe'
if (-not $PrepareOnly -and -not (Test-Path -LiteralPath $command -PathType Leaf)) { throw 'DaVinci 5 command is inaccessible.' }
$sourceIni = Join-Path $sip 'DaVinciConfigurator\Core\DVCfgCmd.ini'
$iniText = [IO.File]::ReadAllText($sourceIni)
$sourceIniHash = (Get-FileHash -LiteralPath $sourceIni -Algorithm SHA256).Hash
$heapPattern = '(?m)^-Xmx[0-9]+[kKmMgG][ \t]*\r?$'
if ([regex]::Matches($iniText,$heapPattern).Count -ne 1) { throw 'Expected one explicit JVM heap limit in DVCfgCmd.ini.' }
New-Item -ItemType Directory -Path $workspace | Out-Null
foreach ($item in Get-ChildItem -LiteralPath $source -Force) {
    Copy-Item -LiteralPath $item.FullName -Destination $workspace -Recurse -Force
}
$privateIni = Join-Path $workspace 'DVCfgCmd.probe.ini'
# Keep the vendor INI untouched. Equinox --launcher.ini selects this private
# copy; the existing process-memory and time guards remain in force.
[IO.File]::WriteAllText($privateIni,([regex]::Replace($iniText,$heapPattern,'-Xmx1024m')),$utf8)
$dpa = Join-Path $workspace $dpaFiles[0].Name
$projectXml = [xml][IO.File]::ReadAllText($dpa)
if ($null -eq $projectXml.ProjectAssistant.Folders.SIP) { throw 'Sample DPA lacks Folders/SIP.' }
$projectXml.ProjectAssistant.Folders.SIP = $sip
$projectXml.Save($dpa)

# SaveProject may touch more than the selected leaf. Check every registered
# ECUC/output/reference path before selecting a candidate, including entries
# after the first usable module. Only the explicitly selected SIP is external.
foreach ($node in @($projectXml.SelectNodes('//EcucSplitter/Splitter | //EcucSplitter/Configuration | //Folders//*[not(*)] | //References//*[not(*)] | //Input/ECUEX'))) {
    $rawPath=if ($node.LocalName -eq 'Splitter') {$node.GetAttribute('File')} else {$node.InnerText}
    if ($node.LocalName -eq 'SIP' -or [string]::IsNullOrWhiteSpace($rawPath)) {continue}
    $registered=[IO.Path]::GetFullPath((Join-Path $workspace $rawPath))
    if (-not $registered.Equals($workspace,[StringComparison]::OrdinalIgnoreCase) -and
        -not $registered.StartsWith($workspace+'\',[StringComparison]::OrdinalIgnoreCase)) {
        throw 'Registered project path escapes disposable root; no native execution.'
    }
}

# Discover one unique, configured Boolean definition from the copied DPA.
$candidate = $null
foreach ($splitter in $projectXml.ProjectAssistant.EcucSplitter.Splitter) {
    $file = [IO.Path]::GetFullPath((Join-Path $workspace ([string]$splitter.File)))
    if (-not $file.StartsWith($workspace + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Sample ECUC reference escapes disposable root.' }
    $doc = [xml][IO.File]::ReadAllText($file)
    $boolValues = @($doc.SelectNodes('//*[local-name()="ECUC-NUMERICAL-PARAM-VALUE"]') | Where-Object {
        $definition = $_.SelectSingleNode('./*[local-name()="DEFINITION-REF"]')
        $value = $_.SelectSingleNode('./*[local-name()="VALUE"]')
        $null -ne $definition -and $definition.GetAttribute('DEST') -eq 'ECUC-BOOLEAN-PARAM-DEF' -and
        $null -ne $value -and $value.InnerText -in @('true','false')
    })
    foreach ($value in $boolValues) {
        $definition = $value.SelectSingleNode('./*[local-name()="DEFINITION-REF"]').InnerText
        if (@($boolValues | Where-Object {$_.SelectSingleNode('./*[local-name()="DEFINITION-REF"]').InnerText -eq $definition}).Count -eq 1) {
            $candidate = @{file=$file;definition=$definition;expected=($value.SelectSingleNode('./*[local-name()="VALUE"]').InnerText -eq 'true')}
            break
        }
    }
    if ($null -ne $candidate) { break }
}
if ($null -eq $candidate) { throw 'No unique configured Boolean is available; no native execution.' }
$request = [ordered]@{schema=1;disposable=$true;phase='prepared';project_file=$dpa;disposable_root=$workspace;
    ecuc_file=$candidate.file;definition_ref=$candidate.definition;expected=$candidate.expected;value=(-not $candidate.expected)}
[IO.File]::WriteAllText((Join-Path $workspace 'lgk-native-probe.json'), ($request | ConvertTo-Json), $utf8)
$scriptDir = Join-Path $workspace 'native-scripts'
New-Item -ItemType Directory -Path $scriptDir | Out-Null
Copy-Item -LiteralPath (Join-Path $central 'assets\experiments\LGKNativeBooleanProbe.dvgroovy') -Destination $scriptDir
if ($PrepareOnly) {
    [ordered]@{prepared=$true;davinci_executed=$false;disposable_root=$workspace;definition_ref=$candidate.definition;jvm_heap_mib=1024} | ConvertTo-Json
    return
}
# Snapshot every copied input file after intentional preparation, before
# DaVinci can load/migrate/save anything. This is evidence, not an undo log.
$baseline = @{}
$sourceBaseline = @{}
foreach ($file in Get-ChildItem -LiteralPath $source -File -Recurse -Force) {
    $relative = $file.FullName.Substring($source.TrimEnd('\').Length+1)
    $sourceBaseline[$relative] = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
    $baseline[$relative] = (Get-FileHash -LiteralPath (Join-Path $workspace $relative) -Algorithm SHA256).Hash
}
[IO.File]::WriteAllText((Join-Path $workspace 'file-baseline.json'),($baseline | ConvertTo-Json),$utf8)
$stdout = Join-Path $workspace 'native.stdout.log'
$stderr = Join-Path $workspace 'native.stderr.log'
# File paths cannot contain quotes on Windows; all quoted arguments end in a
# filename or a newly created directory name, never a trailing backslash.
$arguments = @('--launcher.ini', ('"'+$privateIni+'"'), '--project', ('"'+$dpa+'"'), '--scriptLocations', ('"'+$scriptDir+'"'),
    '--scriptTask', 'LGKNativeBooleanProbe', '--ignoreUserScriptLocations', '--verbose', 'ERROR')
$process = Start-Process -FilePath $command -ArgumentList $arguments -WorkingDirectory $workspace -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
$watch = [Diagnostics.Stopwatch]::StartNew()
$peakPrivateBytes=0L
try {
    while (-not $process.WaitForExit(1000)) {
        if ($watch.Elapsed.TotalSeconds -gt 120) { throw 'Native probe exceeded 120 seconds; logs preserved.' }
        if ([int]$watch.Elapsed.TotalSeconds % 5 -eq 0) {
            $pending = New-Object 'System.Collections.Generic.Queue[int]'
            $pending.Enqueue($process.Id)
            while ($pending.Count -gt 0) {
                $ownedPid = $pending.Dequeue()
                $running = Get-Process -Id $ownedPid -ErrorAction SilentlyContinue
                if ($null -ne $running) {$peakPrivateBytes=[Math]::Max($peakPrivateBytes,$running.PrivateMemorySize64)}
                if ($null -ne $running -and $running.PrivateMemorySize64 -ge 1800MB) { throw 'Owned DaVinci process approaches 2 GiB; logs preserved.' }
                foreach ($child in @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$ownedPid")) { $pending.Enqueue([int]$child.ProcessId) }
            }
        }
    }
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) { throw "DaVinci probe failed, exit=$($process.ExitCode); see $stdout and $stderr" }
    $lines = @(Get-Content -LiteralPath $stdout | Where-Object {$_ -match 'LGK_NATIVE_PROBE_JSON:'})
    if ($lines.Count -ne 1) { throw 'No unique native evidence record; do not claim success.' }
    $evidence = ($lines[0] -split 'LGK_NATIVE_PROBE_JSON:',2)[1] | ConvertFrom-Json
    if (-not $evidence.native_written -or -not $evidence.native_restored -or -not $evidence.saved_written -or -not $evidence.saved_restored -or $evidence.final_value -ne $candidate.expected) { throw 'Native write/restore evidence failed.' }
    $saved = [xml][IO.File]::ReadAllText($candidate.file)
    $final = @($saved.SelectNodes('//*[local-name()="ECUC-NUMERICAL-PARAM-VALUE"]') | Where-Object {$_.SelectSingleNode('./*[local-name()="DEFINITION-REF"]').InnerText -eq $candidate.definition})
    if ($final.Count -ne 1 -or $final[0].SelectSingleNode('./*[local-name()="VALUE"]').InnerText -ne ([string]$candidate.expected).ToLowerInvariant()) { throw 'Saved restored value does not match.' }
    $changed = @()
    foreach ($relative in @($baseline.Keys | Sort-Object)) {
        $inputFile = Join-Path $source $relative
        if (-not (Test-Path -LiteralPath $inputFile -PathType Leaf) -or
            (Get-FileHash -LiteralPath $inputFile -Algorithm SHA256).Hash -ne $sourceBaseline[$relative]) { throw 'Input sample changed during the probe.' }
        $copiedFile = Join-Path $workspace $relative
        $afterHash = if (Test-Path -LiteralPath $copiedFile -PathType Leaf) {(Get-FileHash -LiteralPath $copiedFile -Algorithm SHA256).Hash} else {$null}
        if ($afterHash -ne $baseline[$relative]) {
            $changed += [ordered]@{relative_file=$relative;before_sha256=$baseline[$relative];after_sha256=$afterHash}
        }
    }
    if ((Get-FileHash -LiteralPath $sourceIni -Algorithm SHA256).Hash -ne $sourceIniHash) { throw 'Vendor launcher INI changed during the probe.' }
    $addedConfiguration = @(Get-ChildItem -LiteralPath $workspace -File -Recurse -Force | Where-Object {
        $_.Extension -in @('.dpa','.arxml') -and -not $baseline.ContainsKey($_.FullName.Substring($workspace.Length+1))
    } | ForEach-Object {$_.FullName.Substring($workspace.Length+1)})
    $evidence | Add-Member NoteProperty file_review ([ordered]@{copied_input_count=$baseline.Count;input_files_unchanged=$true;
        vendor_ini_unchanged=$true;changed_copied_files=$changed;added_configuration_files=$addedConfiguration})
    $evidence | Add-Member NoteProperty runtime ([ordered]@{elapsed_seconds=$watch.Elapsed.TotalSeconds;peak_owned_private_bytes=$peakPrivateBytes;jvm_heap_mib=1024})
    [IO.File]::WriteAllText((Join-Path $workspace 'native-evidence.json'), ($evidence | ConvertTo-Json -Depth 4), $utf8)
    $evidence | ConvertTo-Json -Depth 4
} catch {
    $manifest=([IO.File]::ReadAllText((Join-Path $workspace 'lgk-native-probe.json')) | ConvertFrom-Json)
    $failure=[ordered]@{passed=$false;reason=$_.Exception.Message;phase=$manifest.phase;native_script_started=($manifest.phase -ne 'prepared');
        elapsed_seconds=$watch.Elapsed.TotalSeconds;peak_owned_private_bytes=$peakPrivateBytes;jvm_heap_mib=1024;stdout=$stdout;stderr=$stderr}
    [IO.File]::WriteAllText((Join-Path $workspace 'native-failure.json'),($failure | ConvertTo-Json),$utf8)
    throw
} finally {
    if (-not $process.HasExited) {
        & taskkill.exe /PID $process.Id /T /F | Out-Null
    }
    $process.Dispose()
}
