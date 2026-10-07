[CmdletBinding()]
param(
    [Parameter()]
    [string]$RepositoryRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),

    [Parameter()]
    [string]$ExecutablePath = (Join-Path $RepositoryRoot 'target\release\lgk-autosar.exe')
)

$ErrorActionPreference = 'Stop'
$repository = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$wrapper = Join-Path $repository 'scripts\Invoke-LGKAutosar.ps1'
$initializer = Join-Path $repository 'scripts\Initialize-LGKAutosarProject.ps1'
$executable = (Resolve-Path -LiteralPath $ExecutablePath).Path
$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("lgk-autosar-onboarding-中文 空格-" + [Guid]::NewGuid().ToString('N'))
$project = Join-Path $temporaryRoot 'Cfg'
$tool = Join-Path $temporaryRoot 'SIP'
$hostStarted = $false
$activeWrapper = $wrapper
$activeProject = $project
$script:assertionCount = 0

function Write-Utf8([string]$Path, [string]$Content) {
    $parent = Split-Path -Parent $Path
    if (-not (Test-Path -LiteralPath $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    [System.IO.File]::WriteAllText($Path, $Content, [System.Text.UTF8Encoding]::new($false))
}

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) {
        throw "Assertion failed: $Message"
    }
    $script:assertionCount++
}

function Assert-Fails([scriptblock]$Action, [string]$ExpectedText) {
    try {
        & $Action | Out-Null
    } catch {
        $message = $_.Exception.Message
        if ($message -notlike "*$ExpectedText*") {
            throw "Expected failure containing '$ExpectedText', got: $message"
        }
        $script:assertionCount++
        return
    }
    throw "Expected failure containing '$ExpectedText', but the command succeeded"
}

function Test-HostPort([int]$Port = 32483) {
    $client = [System.Net.Sockets.TcpClient]::new()
    try {
        $task = $client.ConnectAsync('127.0.0.1', $Port)
        $task.Wait(200) -and $client.Connected
    } catch {
        $false
    } finally {
        $client.Dispose()
    }
}

try {
    if ((Test-HostPort -Port 32483) -or (Test-HostPort -Port 32484)) {
        throw 'Port 32483 or 32484 is already occupied; stop the existing LGK-AUTOSAR host before this isolated smoke test'
    }

    New-Item -ItemType Directory -Path $project, $tool -Force | Out-Null
    $dpa = Join-Path $project 'PublicExample.dpa'
    $ecuc = Join-Path $project 'Config\ECUC\Public_Com_ecuc.arxml'
    $bswmd = Join-Path $tool 'BSWMD\Com\Com_bswmd.arxml'
    $dvcfg = Join-Path $tool 'DaVinci\Exec\DVCfgCmd.exe'
    Write-Utf8 -Path $dpa -Content @'
<?xml version="1.0"?>
<ProjectAssistant>
  <Folders><ApplicationComponentFolders><ApplicationComponentFolder>Config\Developer</ApplicationComponentFolder></ApplicationComponentFolders></Folders>
  <EcucSplitter>
    <Splitter File=".\Config\ECUC\Public_Com_ecuc.arxml"><Module Name="Com"/></Splitter>
  </EcucSplitter>
</ProjectAssistant>
'@
    Write-Utf8 -Path $ecuc -Content @'
<?xml version="1.0" encoding="UTF-8"?>
<AUTOSAR>
  <ECUC-MODULE-CONFIGURATION-VALUES>
    <SHORT-NAME>Com</SHORT-NAME>
    <DEFINITION-REF DEST="ECUC-MODULE-DEF">/PublicStack/Com</DEFINITION-REF>
    <CONTAINERS><ECUC-CONTAINER-VALUE>
      <SHORT-NAME>ComConfig</SHORT-NAME>
      <DEFINITION-REF DEST="ECUC-PARAM-CONF-CONTAINER-DEF">/PublicStack/Com/ComConfig</DEFINITION-REF>
      <SUB-CONTAINERS><ECUC-CONTAINER-VALUE>
        <SHORT-NAME>PublicSignal</SHORT-NAME>
        <DEFINITION-REF DEST="ECUC-PARAM-CONF-CONTAINER-DEF">/PublicStack/Com/ComConfig/ComSignal</DEFINITION-REF>
        <PARAMETER-VALUES><ECUC-NUMERICAL-PARAM-VALUE>
          <DEFINITION-REF DEST="ECUC-INTEGER-PARAM-DEF">/PublicStack/Com/ComConfig/ComSignal/ComBitPosition</DEFINITION-REF>
          <VALUE>8</VALUE>
        </ECUC-NUMERICAL-PARAM-VALUE></PARAMETER-VALUES>
      </ECUC-CONTAINER-VALUE></SUB-CONTAINERS>
    </ECUC-CONTAINER-VALUE></CONTAINERS>
  </ECUC-MODULE-CONFIGURATION-VALUES>
</AUTOSAR>
'@
    Write-Utf8 -Path $bswmd -Content @'
<?xml version="1.0" encoding="UTF-8"?>
<AUTOSAR><AR-PACKAGES><AR-PACKAGE><SHORT-NAME>PublicStack</SHORT-NAME><ELEMENTS>
  <ECUC-MODULE-DEF><SHORT-NAME>Com</SHORT-NAME><CONTAINERS>
    <ECUC-PARAM-CONF-CONTAINER-DEF><SHORT-NAME>ComConfig</SHORT-NAME><SUB-CONTAINERS>
      <ECUC-PARAM-CONF-CONTAINER-DEF><SHORT-NAME>ComSignal</SHORT-NAME><PARAMETERS>
        <ECUC-INTEGER-PARAM-DEF><SHORT-NAME>ComBitPosition</SHORT-NAME><MIN>0</MIN><MAX>65535</MAX></ECUC-INTEGER-PARAM-DEF>
      </PARAMETERS></ECUC-PARAM-CONF-CONTAINER-DEF>
    </SUB-CONTAINERS></ECUC-PARAM-CONF-CONTAINER-DEF>
  </CONTAINERS></ECUC-MODULE-DEF>
</ELEMENTS></AR-PACKAGE></AR-PACKAGES></AUTOSAR>
'@
    Write-Utf8 -Path $dvcfg -Content ''

    $doctorWatch = [Diagnostics.Stopwatch]::StartNew()
    $initialization = @(& $initializer -ProjectPath $project -ToolPath $tool -ExecutablePath $executable)
    $doctorWatch.Stop()
    $doctor = (($initialization | Out-String) | ConvertFrom-Json)
    Assert-True ($doctor.valid -eq $true) 'initializer doctor must report valid=true'
    Assert-True ($doctor.preflight -eq 'static') 'doctor must label itself as a static preflight'
    Assert-True ($doctor.davinci_executed -eq $false) 'doctor must not claim that DaVinci was executed'
    Assert-True ($doctor.version -eq '0.4.2') 'initializer must use the current release binary'
    Assert-True ($doctorWatch.Elapsed.TotalSeconds -lt 2) 'doctor must complete in under 2 seconds on the public fixture'

    $updateDoctorOutput = @(& $wrapper -ProjectPath $project -ExecutablePath $executable -Request '{"func":"update_project"}' -ValidateOnly)
    $updateDoctor = (($updateDoctorOutput | Out-String) | ConvertFrom-Json)
    Assert-True ($updateDoctor.valid -eq $true) 'update_project must be accepted by wrapper and doctor'
    Assert-True ($updateDoctor.davinci_executed -eq $false) 'update_project doctor must remain non-executing'

    $configPath = Join-Path $project 'lgk-autosar.json'
    $config = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
    Assert-True ($config.PSObject.Properties.Name -notcontains 'project_path') 'portable config must derive project_path from its own directory'
    $configText = [System.IO.File]::ReadAllText($configPath, [System.Text.Encoding]::UTF8)
    $configBom = [System.Text.UTF8Encoding]::new($true)
    [System.IO.File]::WriteAllBytes($configPath, $configBom.GetPreamble() + $configBom.GetBytes($configText))

    # A new user commonly calls the initializer from a parent directory and
    # supplies relative names.  They must be anchored to ProjectPath/ToolPath,
    # not to the shell's current directory.
    $explicitProject = Join-Path $temporaryRoot 'ExplicitCfg'
    New-Item -ItemType Directory -Path $explicitProject -Force | Out-Null
    Copy-Item -LiteralPath $dpa -Destination (Join-Path $explicitProject 'PublicExample.dpa')
    $explicitInitialization = @(& $initializer `
        -ProjectPath $explicitProject `
        -ToolPath $tool `
        -ProjectFile 'PublicExample.dpa' `
        -DavinciCommandPath 'DaVinci\Exec\DVCfgCmd.exe' `
        -ExecutablePath $executable)
    $explicitDoctor = (($explicitInitialization | Out-String) | ConvertFrom-Json)
    Assert-True ($explicitDoctor.valid -eq $true) 'relative explicit project and command paths must pass doctor'
    $explicitConfig = Get-Content -LiteralPath (Join-Path $explicitProject 'lgk-autosar.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    Assert-True ($explicitConfig.project_file -eq 'PublicExample.dpa') 'initializer must store a project-relative DPA path'
    Assert-True ($explicitConfig.davinci_command_path -eq 'DaVinci\Exec\DVCfgCmd.exe') 'initializer must store a tool-relative command path'
    Assert-Fails -ExpectedText 'module is required' -Action {
        & $wrapper -ProjectPath $project -ExecutablePath $executable -Request '{"func":"find_module"}' -ValidateOnly
    }
    Assert-Fails -ExpectedText "Mutating function 'edit_file' must be sent as a standalone request" -Action {
        & $wrapper -ProjectPath $project -ExecutablePath $executable -Request '[{"func":"find_module"},{"func":"edit_file"}]' -ValidateOnly
    }
    Assert-True (-not (Test-HostPort -Port 32483) -and -not (Test-HostPort -Port 32484)) 'wrapper must reject a mutating batch before starting a Host'

    # Regression (2026-08-29 incident): the resident Host must not inherit the
    # caller's output pipe. A backgrounded bash/MSYS pipeline used to stay open
    # after every foreground process had exited because the freshly spawned
    # Host kept the pipe write end. This probe reproduces that topology with a
    # redirected child PowerShell and requires both process exit and stdout
    # EOF within the budget; the pre-fix binary fails this test.
    $probeScriptPath = Join-Path $temporaryRoot 'pipe-regression.ps1'
    # The probe script itself stays pure ASCII and receives every path as a
    # command-line argument: Windows passes arguments as UTF-16, so non-ASCII
    # project paths survive regardless of how either PowerShell edition
    # decodes script files.
    Write-Utf8 -Path $probeScriptPath -Content @'
param(
    [Parameter(Mandatory)][string]$WrapperPath,
    [Parameter(Mandatory)][string]$ProjectPath,
    [Parameter(Mandatory)][string]$ExecutablePath
)
$ErrorActionPreference = 'Stop'
& $WrapperPath -ProjectPath $ProjectPath -ExecutablePath $ExecutablePath -Request '{"func":"find_module","module":"Com"}'
'@
    $probeInfo = New-Object System.Diagnostics.ProcessStartInfo
    $probeInfo.FileName = 'powershell.exe'
    $probeInfo.Arguments = ('-NoProfile -ExecutionPolicy Bypass -File "{0}" -WrapperPath "{1}" -ProjectPath "{2}" -ExecutablePath "{3}"' -f $probeScriptPath, $wrapper, $project, $executable)
    $probeInfo.UseShellExecute = $false
    $probeInfo.RedirectStandardOutput = $true
    $probeInfo.RedirectStandardError = $true
    $probeInfo.CreateNoWindow = $true
    $probeInfo.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $probeInfo.StandardErrorEncoding = [System.Text.Encoding]::UTF8
    $probeWatch = [Diagnostics.Stopwatch]::StartNew()
    $probe = [System.Diagnostics.Process]::Start($probeInfo)
    $hostStarted = $true
    $probeOutputTask = $probe.StandardOutput.ReadToEndAsync()
    if (-not $probe.WaitForExit(60000)) {
        $probe.Kill()
        throw 'pipe regression: wrapper did not exit within 60 seconds through a piped child'
    }
    if (-not $probeOutputTask.Wait(15000)) {
        throw 'pipe regression: caller stdout pipe stayed open after the wrapper exited; the resident Host still inherits caller handles'
    }
    $probeWatch.Stop()
    $probeModule = (($probeOutputTask.Result | Out-String) | ConvertFrom-Json)
    Assert-True ($probeModule.definition_ref -eq '/PublicStack/Com') 'pipe regression: piped wrapper request must return the fixture definition ref'
    Assert-True ($probeWatch.Elapsed.TotalSeconds -lt 30) 'pipe regression: cold piped request must stay well under 30 seconds'

    $localWatch = [Diagnostics.Stopwatch]::StartNew()
    $hostStarted = $true
    $moduleOutput = @(& $wrapper -ProjectPath $project -ExecutablePath $executable -Request '{"func":"find_module","module":"Com","note":"中文路径与 UTF-8"}')
    $localWatch.Stop()
    $module = (($moduleOutput | Out-String) | ConvertFrom-Json)
    Assert-True ($module.definition_ref -eq '/PublicStack/Com') 'find_module must return the fixture definition ref'
    Assert-True ($localWatch.Elapsed.TotalSeconds -lt 5) 'first local request must complete in under 5 seconds'

    $templateOutput = @(& $wrapper -ProjectPath $project -ExecutablePath $executable -Request '{"func":"find_module_template","module":"Com"}')
    $template = (($templateOutput | Out-String) | ConvertFrom-Json)
    Assert-True ($template.PSObject.Properties.Name -notcontains 'definitions') 'default template lookup must stay compact'
    Assert-True ($template.containers[0].name -eq 'ComConfig') 'compact template must preserve container hierarchy'

    $inspectOutput = @(& $wrapper -ProjectPath $project -ExecutablePath $executable -Request '{"func":"inspect_ecuc_containers","module":"Com","container":"ComSignal","params":["ComBitPosition"]}')
    $inspect = (($inspectOutput | Out-String) | ConvertFrom-Json)
    Assert-True (@($inspect).Count -eq 1) 'inspect must return one configured signal'
    Assert-True ([string]$inspect.values.ComBitPosition -eq '8') 'inspect must return ComBitPosition=8'

    $pagedOutput = @(& $wrapper -ProjectPath $project -ExecutablePath $executable -Request '{"func":"inspect_ecuc_containers","module":"Com","container":"ComSignal","paged":true,"limit":1}')
    $paged = (($pagedOutput | Out-String) | ConvertFrom-Json)
    Assert-True ($paged.count -eq 1 -and @($paged.containers).Count -eq 1 -and -not $paged.truncated) 'paged inspection must expose total and completeness'

    Write-Utf8 -Path (Join-Path $project 'Config\Developer\SyntheticMappings.arxml') -Content '<AUTOSAR><SENDER-RECEIVER-TO-SIGNAL-MAPPING><TARGET-DATA-PROTOTYPE-REF>/External/Data</TARGET-DATA-PROTOTYPE-REF><SYSTEM-SIGNAL-REF>/External/Signal</SYSTEM-SIGNAL-REF></SENDER-RECEIVER-TO-SIGNAL-MAPPING><SENDER-RECEIVER-TO-SIGNAL-MAPPING><TARGET-DATA-PROTOTYPE-REF>/External/Data</TARGET-DATA-PROTOTYPE-REF><SYSTEM-SIGNAL-REF>/External/Signal</SYSTEM-SIGNAL-REF></SENDER-RECEIVER-TO-SIGNAL-MAPPING></AUTOSAR>'
    $mappingOutput = @(& $wrapper -ProjectPath $project -ExecutablePath $executable -Request '{"func":"inspect_autosar_mapping","category":"data","summary_only":true}')
    $mapping = (($mappingOutput | Out-String) | ConvertFrom-Json)
    Assert-True ($mapping.count -eq 2 -and @($mapping.rows).Count -eq 0) 'mapping summary must count anonymous rows without returning each one'
    Assert-True ($mapping.unresolved_references_in_scope -eq 4) 'mapping must report scope-limited evidence rather than a DaVinci validation result'

    Write-Utf8 -Path (Join-Path $project 'Config\Developer\PublicTypes.arxml') -Content '<AUTOSAR><AR-PACKAGES><AR-PACKAGE><SHORT-NAME>PublicTypes</SHORT-NAME><ELEMENTS><IMPLEMENTATION-DATA-TYPE><SHORT-NAME>Byte</SHORT-NAME></IMPLEMENTATION-DATA-TYPE></ELEMENTS></AR-PACKAGE></AR-PACKAGES></AUTOSAR>'
    $aswFile = Join-Path $project 'Config\Developer\Authored.arxml'
    $aswRequest = @{func='write_asw_bundle';file=$aswFile;expected=$null;preview=$true;bundle=@{package='Authored';
        types=@(@{kind='alias';name='Counter';type_ref='/PublicTypes/Byte'});
        interfaces=@(@{kind='sender_receiver';name='Values';data_elements=@(@{name='Count';type_ref='/Authored/Counter'})});
        components=@(@{name='Producer';ports=@(@{name='Out';direction='provide';interface_ref='/Authored/Values'})})}}
    $aswPreview = (@(& $wrapper -ProjectPath $project -ExecutablePath $executable -Request ($aswRequest | ConvertTo-Json -Depth 10 -Compress)) | Out-String) | ConvertFrom-Json
    Assert-True ($aswPreview.preview -and -not (Test-Path -LiteralPath $aswFile)) 'ASW bundle preview must not write a file'
    Assert-True (-not $aswPreview.davinci_validated) 'ASW disk preview must not claim native validation'
    $aswRequest.preview=$false
    & $wrapper -ProjectPath $project -ExecutablePath $executable -Request ($aswRequest | ConvertTo-Json -Depth 10 -Compress) | Out-Null
    Assert-True (Test-Path -LiteralPath $aswFile) 'ASW bundle must create a registered input file'
    Assert-Fails { & $wrapper -ProjectPath $project -ExecutablePath $executable -Request ($aswRequest | ConvertTo-Json -Depth 10 -Compress) | Out-Null } 'absent file'
    Assert-Fails { & $wrapper -ProjectPath $project -ExecutablePath $executable -Request '[{"func":"find_module","module":"Com"},{"func":"write_asw_bundle"}]' | Out-Null } "Mutating function 'write_asw_bundle'"
    $aswInspect = (@(& $wrapper -ProjectPath $project -ExecutablePath $executable -Request '{"func":"inspect_autosar_model","path_prefix":"/Authored","summary_only":true}') | Out-String) | ConvertFrom-Json
    Assert-True ($aswInspect.total -eq 6) 'ASW created objects must be discoverable through the wrapper'
    $aswDelete=@{func='write_asw_bundle';file=$aswFile;expected=[Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes($aswFile));delete=$true;preview=$true}
    & $wrapper -ProjectPath $project -ExecutablePath $executable -Request ($aswDelete | ConvertTo-Json -Depth 4 -Compress) | Out-Null
    Assert-True (Test-Path -LiteralPath $aswFile) 'ASW deletion preview must preserve the file'
    $aswDelete.preview=$false
    & $wrapper -ProjectPath $project -ExecutablePath $executable -Request ($aswDelete | ConvertTo-Json -Depth 4 -Compress) | Out-Null
    Assert-True (-not (Test-Path -LiteralPath $aswFile)) 'ASW deletion must require and match complete old content'

    # Native preparation is synthetic only: it must never invoke the empty
    # DaVinci placeholder or reuse a customer project as a writable fixture.
    $nativeSample = Join-Path $temporaryRoot 'NativeSample'
    $nativeWork = Join-Path $temporaryRoot 'NativeDisposable'
    Write-Utf8 -Path (Join-Path $nativeSample 'Native.dpa') -Content '<ProjectAssistant><Folders><SIP>..\SIP</SIP></Folders><EcucSplitter><Splitter File="Native.arxml"><Module Name="Com"/></Splitter></EcucSplitter></ProjectAssistant>'
    Write-Utf8 -Path (Join-Path $nativeSample 'Native.arxml') -Content '<AUTOSAR><ECUC-NUMERICAL-PARAM-VALUE><DEFINITION-REF DEST="ECUC-BOOLEAN-PARAM-DEF">/PublicStack/Com/Flag</DEFINITION-REF><VALUE>false</VALUE></ECUC-NUMERICAL-PARAM-VALUE></AUTOSAR>'
    $sourceHash = (Get-FileHash -LiteralPath (Join-Path $nativeSample 'Native.arxml')).Hash
    $nativeDriver = Join-Path $repository 'scripts\Invoke-LGKNativeBooleanProbe.ps1'
    $nativeIni = Join-Path $tool 'DaVinciConfigurator\Core\DVCfgCmd.ini'
    Write-Utf8 -Path $nativeIni -Content "-application`npublic.synthetic.application`n-vmargs`n-Xmx16384m`n-Dfile.encoding=UTF-8`n"
    $sourceIniHash = (Get-FileHash -LiteralPath $nativeIni).Hash
    $nativeOutput = @(& $nativeDriver -SampleProjectPath $nativeSample -ToolPath $tool -WorkRoot $nativeWork -PrepareOnly)
    $nativePrepared = (($nativeOutput | Out-String) | ConvertFrom-Json)
    Assert-True ($nativePrepared.prepared -and -not $nativePrepared.davinci_executed) 'native prepare-only must not launch DaVinci'
    Assert-True ([IO.File]::ReadAllText((Join-Path $nativeWork 'DVCfgCmd.probe.ini')) -eq [IO.File]::ReadAllText($nativeIni).Replace('-Xmx16384m','-Xmx1024m') -and $nativePrepared.jvm_heap_mib -eq 1024) 'native private launcher must change only the JVM heap argument'
    Assert-True ((Get-FileHash -LiteralPath $nativeIni).Hash -eq $sourceIniHash) 'native preparation must preserve the vendor launcher INI'
    $nativeManifest = ([IO.File]::ReadAllText((Join-Path $nativeWork 'lgk-native-probe.json')) | ConvertFrom-Json)
    Assert-True ($nativeManifest.phase -eq 'prepared' -and -not $nativeManifest.expected -and $nativeManifest.value) 'native manifest must preserve old value and stage a Boolean change'
    Assert-True ((Get-FileHash -LiteralPath (Join-Path $nativeSample 'Native.arxml')).Hash -eq $sourceHash) 'native preparation must preserve input sample bytes'
    Assert-Fails { & $nativeDriver -SampleProjectPath $nativeSample -ToolPath $tool -WorkRoot $nativeWork -PrepareOnly | Out-Null } 'already exists'
    Assert-Fails { & $nativeDriver -SampleProjectPath $nativeSample -ToolPath $tool -WorkRoot (Join-Path $tool 'UnsafeProbe') -PrepareOnly | Out-Null } 'outside the source'
    $escapeDpa = Join-Path $nativeSample 'Native.dpa'
    Write-Utf8 -Path $escapeDpa -Content ([IO.File]::ReadAllText($escapeDpa).Replace('</EcucSplitter>','<Splitter File="..\Outside.arxml"><Module Name="Os"/></Splitter></EcucSplitter>'))
    Assert-Fails { & $nativeDriver -SampleProjectPath $nativeSample -ToolPath $tool -WorkRoot (Join-Path $temporaryRoot 'NativeEscapeRejected') -PrepareOnly | Out-Null } 'escapes disposable root'
    Write-Utf8 -Path $nativeIni -Content "-vmargs`n-Dfile.encoding=UTF-8`n"
    $invalidHeapWork = Join-Path $temporaryRoot 'NativeHeapRejected'
    Assert-Fails { & $nativeDriver -SampleProjectPath $nativeSample -ToolPath $tool -WorkRoot $invalidHeapWork -PrepareOnly | Out-Null } 'one explicit JVM heap limit'
    Assert-True (-not (Test-Path -LiteralPath $invalidHeapWork)) 'unrecognized launcher heap must fail before copying the sample'

    $diffRight = Join-Path $project 'Config\ECUC\Public_Com_changed.arxml'
    Write-Utf8 -Path $diffRight -Content ([IO.File]::ReadAllText($ecuc).Replace('<VALUE>8</VALUE>', '<VALUE>16</VALUE>'))
    $diffRequest = [ordered]@{
        func = 'diff_ecuc'
        module = 'Com'
        left = 'Config\ECUC\Public_Com_ecuc.arxml'
        right = 'Config\ECUC\Public_Com_changed.arxml'
        path_prefix = 'Com/ComConfig/PublicSignal'
        limit = 1
    } | ConvertTo-Json -Compress
    $diffOutput = @(& $wrapper -ProjectPath $project -ExecutablePath $executable -Request $diffRequest)
    $diff = (($diffOutput | Out-String) | ConvertFrom-Json)
    Assert-True ($diff.total -eq 1 -and $diff.counts.modify -eq 1) 'diff_ecuc must report one semantic modification'
    Assert-True ($diff.changes[0].old -eq '8' -and $diff.changes[0].new -eq '16') 'diff_ecuc must return compact old/new values'
    Assert-True ($diff.truncated -eq $false) 'diff_ecuc must not mark a complete bounded result as truncated'

    $semanticEditRequest = [ordered]@{
        func = 'set_ecuc_value'
        module = 'Com'
        container_path = 'Com/ComConfig/PublicSignal'
        parameter = 'ComBitPosition'
        expected = '8'
        value = '16'
    } | ConvertTo-Json -Compress
    $semanticEditOutput = @(& $wrapper -ProjectPath $project -ExecutablePath $executable -Request $semanticEditRequest)
    $semanticEdit = (($semanticEditOutput | Out-String) | ConvertFrom-Json)
    Assert-True ($semanticEdit.changed -eq $true -and $semanticEdit.old -eq '8' -and $semanticEdit.new -eq '16') 'set_ecuc_value must apply one verified semantic edit'
    Assert-True ([IO.File]::ReadAllText($ecuc).Contains('<VALUE>16</VALUE>')) 'set_ecuc_value must update the selected saved value'
    $semanticRestoreRequest = [ordered]@{
        func = 'set_ecuc_value'
        module = 'Com'
        container_path = '/ComConfig/PublicSignal'
        parameter = 'ComBitPosition'
        expected = '16'
        value = '8'
    } | ConvertTo-Json -Compress
    & $wrapper -ProjectPath $project -ExecutablePath $executable -Request $semanticRestoreRequest | Out-Null
    Assert-True ([IO.File]::ReadAllText($ecuc).Contains('<VALUE>8</VALUE>')) 'set_ecuc_value must accept a module-less canonical container path'

    $generatedDelivery = Join-Path $temporaryRoot 'Generated\Com_Cfg.h'
    $compiledDelivery = Join-Path $temporaryRoot 'Proj_Code\Com_Cfg.h'
    Write-Utf8 -Path $generatedDelivery -Content '#define COM_CONFIG_VALUE 8'
    Write-Utf8 -Path $compiledDelivery -Content '#define COM_CONFIG_VALUE 8'
    $deliveryRequest = [ordered]@{
        func = 'verify_delivery'
        root = $temporaryRoot
        checks = @([ordered]@{
            path = 'Proj_Code\Com_Cfg.h'
            same_as = 'Generated\Com_Cfg.h'
            must_contain = @('COM_CONFIG_VALUE 8')
            must_not_contain = @('COM_CONFIG_VALUE 7')
        })
    } | ConvertTo-Json -Compress -Depth 5
    $deliveryOutput = @(& $wrapper -ProjectPath $project -ExecutablePath $executable -Request $deliveryRequest)
    $delivery = (($deliveryOutput | Out-String) | ConvertFrom-Json)
    Assert-True ($delivery.passed -eq $true) 'verify_delivery must accept synchronized generated output'
    Assert-True ($delivery.checks[0].synchronized -eq $true) 'verify_delivery must compare generated and compiled files'

    $failureRequest = @{ func = 'verify_delivery'; root = $temporaryRoot; checks = @(@{path = 'missing-output.h'; must_contain = @('EXPECTED_SYMBOL')}) } | ConvertTo-Json -Depth 5 -Compress
    $failureText = $null
    try {
        & $wrapper -ProjectPath $project -ExecutablePath $executable -Request $failureRequest | Out-Null
    } catch {
        $failureText = $_.Exception.Message
    }
    Assert-True ($null -ne $failureText) 'structured verification failure must still fail the wrapper'
    $failure = $failureText.Substring($failureText.IndexOf('{')) | ConvertFrom-Json
    Assert-True ($failure.code -eq 'DELIVERY_VERIFICATION_FAILED') 'failure code must survive Host CLI and wrapper'
    Assert-True ($failure.details.failed_checks[0].missing_required[0] -eq 'EXPECTED_SYMBOL') 'first failure must include missing symbol without a second request'

    $bomRequest = Join-Path $temporaryRoot 'request-with-bom.json'
    $bomEncoding = [System.Text.UTF8Encoding]::new($true)
    $bomBytes = $bomEncoding.GetPreamble() + $bomEncoding.GetBytes('{"func":"find_module","module":"Com"}')
    [System.IO.File]::WriteAllBytes($bomRequest, $bomBytes)
    $bomOutput = @(& $wrapper -ProjectPath $project -ExecutablePath $executable -RequestFile $bomRequest)
    $bomModule = (($bomOutput | Out-String) | ConvertFrom-Json)
    Assert-True ($bomModule.definition_ref -eq '/PublicStack/Com') 'UTF-8 BOM request files must be accepted end to end'

    Assert-Fails -ExpectedText 'module is required' -Action {
        & $wrapper -ProjectPath $project -ExecutablePath $executable -Request '{"func":"generate_code"}'
    }

    $failingRequest = Join-Path $temporaryRoot 'failing-request.json'
    Write-Utf8 -Path $failingRequest -Content '{"func":"find_module","module":"Missing"}'
    $failureText = $null
    try {
        & $wrapper -ProjectPath $project -ExecutablePath $executable -RequestFile $failingRequest
    } catch {
        $failureText = $_.Exception.Message
    }
    Assert-True ($null -ne $failureText -and $failureText.Contains("request: $failingRequest")) 'module failure must preserve request context'
    $failure = $failureText.Substring($failureText.IndexOf('{')) | ConvertFrom-Json
    Assert-True ($failure.code -eq 'MODULE_NOT_FOUND') 'unknown module must have a stable error code'
    Assert-True ($failure.details.candidates -contains 'Com') 'unknown module must return a real project candidate'

    Assert-Fails -ExpectedText 'file changed' -Action {
        $request = [ordered]@{
            func = 'edit_file'
            path = $ecuc
            expected = @{ '14' = '          <VALUE>7</VALUE>' }
            edits = @{ '14' = '          <VALUE>9</VALUE>' }
        } | ConvertTo-Json -Compress -Depth 5
        & $wrapper -ProjectPath $project -ExecutablePath $executable -Request $request
    }

    $editRequest = [ordered]@{
        func = 'edit_file'
        path = $ecuc
        expected = @{ '14' = '          <VALUE>8</VALUE>' }
        edits = @{ '14' = '          <VALUE>9</VALUE>' }
    } | ConvertTo-Json -Compress -Depth 5
    & $wrapper -ProjectPath $project -ExecutablePath $executable -Request $editRequest | Out-Null
    $edited = [System.IO.File]::ReadAllText($ecuc)
    Assert-True ($edited.Contains('<VALUE>9</VALUE>')) 'edit_file must apply when expected text is current'

    Write-Utf8 -Path (Join-Path $project 'Second.dpa') -Content '<ProjectAssistant/>'
    Assert-Fails -ExpectedText 'multiple .dpa files' -Action {
        & $wrapper -ProjectPath $project -ExecutablePath $executable -Request '{"func":"get_errors_list"}' -ValidateOnly
    }
    Remove-Item -LiteralPath (Join-Path $project 'Second.dpa') -Force

    $secondDvcfg = Join-Path $tool 'Other\DVCfgCmd.exe'
    Write-Utf8 -Path $secondDvcfg -Content ''
    Assert-Fails -ExpectedText 'multiple DVCfgCmd.exe' -Action {
        & $wrapper -ProjectPath $project -ExecutablePath $executable -Request '{"func":"get_errors_list"}' -ValidateOnly
    }
    Remove-Item -LiteralPath $secondDvcfg -Force

    & $wrapper -ProjectPath $project -ExecutablePath $executable -Request '{"func":"shutdown_host"}' | Out-Null
    $hostStarted = $false
    Assert-True (-not (Test-HostPort -Port 32483)) 'source-tree host must release the business port before package execution'
    Assert-True (-not (Test-HostPort -Port 32484)) 'source-tree host must release the health port before package execution'

    $package = Join-Path $temporaryRoot 'release-package'
    & (Join-Path $repository 'scripts\Sync-LGKAutosarPackage.ps1') `
        -SourceRoot $repository `
        -DestinationRoot $package `
        -IncludeBinaries | Out-Null
    Assert-True (Test-Path -LiteralPath (Join-Path $package 'lgk-autosar\lgk-autosar.exe') -PathType Leaf) 'release package must include the CLI'
    Assert-True (Test-Path -LiteralPath (Join-Path $package 'lgk-autosar\lgk-autosar-host.exe') -PathType Leaf) 'release package must include the matching Host'
    Assert-True (Test-Path -LiteralPath (Join-Path $package 'lgk-autosar\Invoke-LGKAutosar.ps1') -PathType Leaf) 'release package must include the runtime wrapper'
    Assert-True (Test-Path -LiteralPath (Join-Path $package 'README.md') -PathType Leaf) 'release package must include the short install guide'
    Assert-True (Test-Path -LiteralPath (Join-Path $package 'LICENSE') -PathType Leaf) 'release package must include LICENSE'
    Assert-True (Test-Path -LiteralPath (Join-Path $package 'NOTICE') -PathType Leaf) 'release package must include NOTICE'
    Assert-True (Test-Path -LiteralPath (Join-Path $package 'lgk-autosar\SKILL.md') -PathType Leaf) 'release package must include the installable Skill'
    Assert-True (Test-Path -LiteralPath (Join-Path $package 'lgk-autosar\AGENTS.md') -PathType Leaf) 'release package must include cross-agent instructions'
    Assert-True (Test-Path -LiteralPath (Join-Path $package 'test\\Run-ExeSelfTest.ps1') -PathType Leaf) 'release package must include the end-user EXE self-test'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $package 'tests') -PathType Container)) 'release package must not include development tests'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $package 'src') -PathType Container)) 'release package must not include Rust source'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $package '.github') -PathType Container)) 'release package must not include CI configuration'
    Assert-Fails -ExpectedText 'must be new or empty' -Action {
        & (Join-Path $repository 'scripts\Sync-LGKAutosarPackage.ps1') `
            -SourceRoot $repository `
            -DestinationRoot $package
    }

    $packageProject = Join-Path $temporaryRoot 'PackagedCfg'
    New-Item -ItemType Directory -Path $packageProject -Force | Out-Null
    Copy-Item -LiteralPath $dpa -Destination (Join-Path $packageProject 'PublicExample.dpa')
    Copy-Item -LiteralPath (Join-Path $project 'Config') -Destination $packageProject -Recurse
    $packageRuntime = Join-Path $package 'lgk-autosar'
    $packageInitializer = Join-Path $packageRuntime 'Initialize-LGKAutosarProject.ps1'
    $packageWrapper = Join-Path $packageRuntime 'Invoke-LGKAutosar.ps1'
    $packageDoctorOutput = @(& $packageInitializer -ProjectPath $packageProject -ToolPath $tool)
    $packageDoctor = (($packageDoctorOutput | Out-String) | ConvertFrom-Json)
    Assert-True ($packageDoctor.version -eq '0.4.2') 'packaged initializer must use packaged binaries by default'

    $activeWrapper = $packageWrapper
    $activeProject = $packageProject
    $hostStarted = $true
    $packageToken = Join-Path $packageRuntime '.lgk-autosar\host.token'
    Write-Utf8 -Path $packageToken -Content 'invalid'
    $packageModuleOutput = @(& $packageWrapper -ProjectPath $packageProject -Request '{"func":"find_module","module":"Com"}')
    $packageModule = (($packageModuleOutput | Out-String) | ConvertFrom-Json)
    Assert-True ($packageModule.definition_ref -eq '/PublicStack/Com') 'packaged wrapper must execute a real local request'
    $repairedToken = ([System.IO.File]::ReadAllText($packageToken)).Trim()
    Assert-True ($repairedToken.Length -eq 64 -and $repairedToken -match '^[0-9a-f]+$') 'packaged CLI must replace an invalid resident token'
    Write-Utf8 -Path (Join-Path $packageProject 'lgk-autosar.json') -Content '{invalid'
    & $packageWrapper -ProjectPath $packageProject -Request '{"func":"shutdown_host"}' | Out-Null
    $hostStarted = $false
    Assert-True (-not (Test-HostPort -Port 32483)) 'packaged shutdown_host must release the business port'
    Assert-True (-not (Test-HostPort -Port 32484)) 'packaged shutdown_host must release the health port'

    [pscustomobject]@{
        valid = $true
        doctor_ms = [Math]::Round($doctorWatch.Elapsed.TotalMilliseconds)
        first_local_request_ms = [Math]::Round($localWatch.Elapsed.TotalMilliseconds)
        tests = $script:assertionCount
    } | ConvertTo-Json
} finally {
    if ($hostStarted) {
        try {
            & $activeWrapper -ProjectPath $activeProject -Request '{"func":"shutdown_host"}' | Out-Null
        } catch {
            Write-Warning "Failed to stop smoke-test host: $($_.Exception.Message)"
        }
    }
    $tempBase = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    $resolvedTemporary = [System.IO.Path]::GetFullPath($temporaryRoot)
    if ($resolvedTemporary.StartsWith($tempBase, [StringComparison]::OrdinalIgnoreCase) -and
        (Split-Path -Leaf $resolvedTemporary).StartsWith('lgk-autosar-onboarding-')) {
        if (Test-Path -LiteralPath $resolvedTemporary) {
            # A just-closed Windows process may retain its current directory
            # for a few milliseconds.  Leave the fixture before deleting it,
            # retry briefly, and never hide the real test failure with a cleanup
            # race error.
            Set-Location -LiteralPath $repository
            foreach ($attempt in 1..20) {
                try {
                    Remove-Item -LiteralPath $resolvedTemporary -Recurse -Force -ErrorAction Stop
                    break
                } catch {
                    if ($attempt -eq 20) {
                        Write-Warning "Failed to remove smoke-test directory after retries: $resolvedTemporary"
                    } else {
                        Start-Sleep -Milliseconds 100
                    }
                }
            }
        }
    } else {
        Write-Warning "Refusing to remove unexpected temporary path: $resolvedTemporary"
    }
}
