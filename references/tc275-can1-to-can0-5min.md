# TC275 CAN1 → CAN0 five-minute runbook

Use this runbook only for the verified TC275 board layout below. It is designed
for a normal, narrowly scoped CAN1-to-CAN0 repair; it is not a generic way to
change every CAN project.

## Contents

1. [Scope and stop conditions](#scope-and-stop-conditions)
2. [Known-good electrical mapping](#known-good-electrical-mapping)
3. [Five-minute execution plan](#five-minute-execution-plan)
4. [Edit the source configuration](#edit-the-source-configuration)
5. [Generate the two affected modules](#generate-the-two-affected-modules)
6. [Synchronize only compiled output](#synchronize-only-compiled-output)
7. [Generated-code acceptance checks](#generated-code-acceptance-checks)
8. [Runtime handoff and Git](#runtime-handoff-and-git)
9. [Failure patterns](#failure-patterns)

## Scope and stop conditions

This procedure applies only when all items below are true:

- MCU: Infineon TC275.
- Target controller: CAN0 / `Node0`.
- Board RX pin: P20.7 (`RXDCAN0B`).
- Board TX pin: P20.8 (`TXDCAN0`).
- Physical transceiver: TJA1040, with active-low NEN/STB on P20.6.
- Expected network speed: 500 kbit/s unless the current approved DBC/configuration
  says otherwise.

Stop before changing anything if the schematic or ECUC names identify another
pin, transceiver, derivative, or bitrate. `Node0` does **not** imply an RXSEL
value. Do not copy these values to CAN1, a different RX pad, or a TJA1043 board.

The five-minute target assumes DaVinci and EB tresos are installed, the DPA is
valid, and no generator is already running. If a tool fails or has not advanced
by five minutes, stop and report the exact command/report failure. Do not retry
full-project generation, edit generated C by hand, or guess a replacement pin.

## Known-good electrical mapping

| Concern | Required value | Why it matters |
|---|---|---|
| CAN controller | `Node0` | Selects TC275 CAN0. |
| CAN base address | `0xF0018200` | Confirms CAN0 register block. |
| CAN source | `CanSRC = 0` | Driver source selection for this Node0 setup. |
| RX pin | P20.7 `RXDCAN0B` | Board input from TJA1040 RXD. |
| RX selector | `CanIOPort = b_001` | Selects the P20.7 CAN0-B input path. |
| Generated RX selector | `Can_InitPortSel[0] = 1u` | Independent generated proof of the RX setting. |
| TX pin | P20.8 `TXDCAN0` | Board output to TJA1040 TXD. |
| TX port mode | Output, push-pull, alternate function 5 | GPIO input produces no CAN transmission. |
| Transceiver enable | P20.6 NEN/STB | TJA1040 enable/standby control. |
| Enable port mode | GPIO output, push-pull, initial low | NEN/STB is active low; input mode leaves the bus silent. |

## Five-minute execution plan

| Time | Required action | Exit evidence |
|---|---|---|
| 0:00–0:30 | Record Git state, read schematic and current controller/Port source. | Mapping above matches the actual board. |
| 0:30–1:30 | Make the two source changes: Can ECUC and Port.xdm. | Narrow diffs show only intended controller/pin values. |
| 1:30–2:15 | Generate only `Port_AurixAS403` in EB tresos. | Generator exits with zero errors/warnings. |
| 2:15–3:00 | Generate only `Can` through LGK-Vector. | Newest report says Can validation/generation succeeded. |
| 3:00–3:45 | Sync only Can and Port generated files used by the compile project. | Destination files changed as expected. |
| 3:45–4:30 | Assert `Can_InitPortSel` and three Port20 pin modes. | All generated values match this guide. |
| 4:30–5:00 | Review Git scope and hand off for the user's build/bus test. | No unrelated file is staged; status is recorded. |

Do not compile or flash if the user said they will build/test themselves. A tool
agent's job here is to deliver a source-and-generated configuration that can be
verified; a completed generator is not proof of bus traffic.

## Edit the source configuration

### 1. Establish the repository and preserve user work

From the TC275 repository root, record the state before any edit:

```powershell
git status --short
git diff -- Proj_Config/Bsw_Config/Cfg/Config/ECUC/TC27x_Can_ecuc.arxml
git diff -- Proj_Config/Mcal_Config/Cfg/config/Port.xdm
```

Never clean, reset, or stage existing user changes. Generated build folders,
IDE metadata, logs, and a user's `.cproject` edit are not part of this repair
unless the user explicitly puts them in scope.

### 2. Change Can ECUC through LGK-Vector

Save and close the DaVinci project before a mutating request. Use LGK-Vector to
inspect the actual controller container first, then make one narrow `edit_file`
request with exact `expected` values. The source normally lives at:

```text
Proj_Config/Bsw_Config/Cfg/Config/ECUC/TC27x_Can_ecuc.arxml
```

The target controller must have this semantic state:

```text
Physical Node / controller: Node0
Controller Base Address:    0xF0018200
Controller Id:              0
CanSRC:                     0
Receive Input Selection:    b_001
```

Keep existing message objects, baudrate container, and unrelated CAN networks
unchanged. In particular, do **not** set `b_000` simply because the controller
is Node0. `b_001` is the hardware RX multiplexer selection for P20.7
`RXDCAN0B`.

Example inspection call (adjust the container name only after inspection):

```powershell
& 'C:\Users\l\.codex\skills\lgk-vector\scripts\Invoke-LGKVector.ps1' `
  -ProjectPath 'D:\Project\AutosarSpace\Tc275-V1.3\Proj_Config\Bsw_Config\Cfg' `
  -Request '{"func":"inspect_ecuc_containers","module":"Can","container":"CanController","params":["CanControllerId","CanControllerBaseAddress","CanControllerRxInputSelection"]}'
```

Use the inspected range and exact old values in LGK's standalone `edit_file`
request. If its expected-value guard rejects the edit, re-read the source and
stop; do not bypass the guard with a broad XML replacement.

### 3. Change MCAL Port source in EB tresos configuration

The source is not the generated `Port_PBCfg.c`. Edit:

```text
Proj_Config/Mcal_Config/Cfg/config/Port.xdm
```

Set these existing Port20 pin containers:

| Pin ID | Board function | Direction/mode required |
|---:|---|---|
| 326 / P20.6 | TJA1040 NEN/STB | output, push-pull, GPIO; initial low |
| 327 / P20.7 | RXDCAN0B | input, pull-up, GPIO |
| 328 / P20.8 | TXDCAN0 | output, push-pull, alternate function 5 |

Do not turn P20.7 into an alternate output. It is the physical receive input.
Do not leave P20.6 or P20.8 as inputs: both cases can compile and generate
successfully while the physical bus remains silent.

When using a tool to edit an XML source, make one pin-scoped change and inspect
the resulting source immediately. Do not make a permanent project-local LGK
configuration or copy LGK binaries into the AUTOSAR project in order to edit
MCAL files.

## Generate the two affected modules

### 1. Generate Port only with EB tresos

Use a fresh temporary workspace; do not use the permanent configuration folder
as an EB workspace, because it creates `.metadata`. The EB project name is
`TC275`, not the last directory name `Cfg`.

```powershell
$workspace = New-Item -ItemType Directory -Path (Join-Path ([IO.Path]::GetTempPath()) ('tc275-port-gen-' + [guid]::NewGuid().ToString('N')))
$tresos = 'D:\EB\tresos16\bin\tresos_cmd.bat'
$mcalProject = 'D:\Project\AutosarSpace\Tc275-V1.3\Proj_Config\Mcal_Config\Cfg'
& $tresos -data $workspace.FullName importProject $mcalProject
if ($LASTEXITCODE -ne 0) { throw 'EB importProject failed; do not continue.' }
& $tresos -data $workspace.FullName generate -g Port_AurixAS403 TC275
if ($LASTEXITCODE -ne 0) { throw 'EB Port generation failed; do not sync output.' }
```

The proven run completed in approximately four seconds. Retain the console
output as evidence. Do not run an unrelated MCAL full generation to repair only
these three pins.

### 2. Generate Can only with LGK-Vector

After the DaVinci GUI is closed, submit one standalone request:

```powershell
& 'C:\Users\l\.codex\skills\lgk-vector\scripts\Invoke-LGKVector.ps1' `
  -ProjectPath 'D:\Project\AutosarSpace\Tc275-V1.3\Proj_Config\Bsw_Config\Cfg' `
  -Request '{"func":"generate_code","module":"Can"}'
```

Read the newest DaVinci generation report. Accept it only if it has zero
validation errors and the `Can` generation phase is successful. The wrapper's
“completed” text alone is insufficient. End the resident host with a separate
`shutdown_host` request when the operation is finished.

The proven wrapper call took roughly 29 seconds. If a generator is stalled,
report the report/log location and do not retry as `module:"all"`.

## Synchronize only compiled output

Copy only generated files proven to be produced by these two changed modules.
Never synchronize a full `Gen` tree.

| Generated source | Compile-project destination |
|---|---|
| `Proj_Config/Bsw_Config/Gen/GenData/Can_Lcfg.c` | `Proj_Code/_01_BSW/Gen/GenData/Can_Lcfg.c` |
| `Proj_Config/Mcal_Config/Gen/inc/Port_Cfg.h` | `Proj_Code/_03_MCAL/Gen/inc/Port_Cfg.h` |
| `Proj_Config/Mcal_Config/Gen/src/Port_PBCfg.c` | `Proj_Code/_03_MCAL/Gen/src/Port_PBCfg.c` |
| `Proj_Config/Mcal_Config/Gen/inc/Port_Cfg.h` | `Proj_Config/Bsw_Config/Gen/GenData/Port_Cfg.h` when this mirror is tracked |
| `Proj_Config/Mcal_Config/Gen/src/Port_PBCfg.c` | `Proj_Config/Bsw_Config/Gen/GenData/Port_PBCfg.c` when this mirror is tracked |

Before copying, compare source and destination. Copy a file only when it is a
direct affected output and its diff matches the intended change. Generated
formatting and ordering must not be manually “cleaned up.”

## Generated-code acceptance checks

Run the LGK delivery gate against the **compiled destinations**, not merely the
generator output folder:

```powershell
$request = @{
  func = 'verify_delivery'
  root = 'D:\Project\AutosarSpace\Tc275-V1.3'
  checks = @(
    @{
      path = 'Proj_Code\_01_BSW\Gen\GenData\Can_Lcfg.c'
      same_as = 'Proj_Config\Bsw_Config\Gen\GenData\Can_Lcfg.c'
      must_contain = @('CanIsr_0', '0xF0018200u')
      must_not_contain = @('CanIsr_1')
    },
    @{
      path = 'Proj_Code\_03_MCAL\Gen\src\Port_PBCfg.c'
      same_as = 'Proj_Config\Mcal_Config\Gen\src\Port_PBCfg.c'
    }
  )
} | ConvertTo-Json -Compress -Depth 6
& 'C:\Users\l\.codex\skills\lgk-vector\scripts\Invoke-LGKVector.ps1' `
  -ProjectPath 'D:\Project\AutosarSpace\Tc275-V1.3\Proj_Config\Bsw_Config\Cfg' `
  -Request $request
```

Require `passed:true`, then inspect the Port20 block for the three adjacent pin
rows below. Exact file synchronization alone cannot prove that a chosen pin
mode is electrically correct.

Accept the change only when all of these are true:

- `Can_Lcfg.c` selects `Can_InitPortSel[0] = 1u` (or its equivalent formatted
  initializer), uses `CanIsr_0`, and contains `0xF0018200u`.
- P20.6 / pin 6 is `OUT`, `PUSHPULL`, `GPIO` and begins low.
- P20.7 / pin 7 is `IN`, pull-up, `GPIO`.
- P20.8 / pin 8 is `OUT`, `PUSHPULL`, `ALT5`.
- No destination still represents P20.8 as a GPIO input or P20.6 as an input.

These assertions are mandatory because the earlier faulty conversion had a
correct-looking Node0 but `Can_InitPortSel=0`, P20.8 as GPIO input, and P20.6
as input. It compiled, flashed, and transmitted no frames.

## Runtime handoff and Git

If the user performs the build/bus test, provide the exact scope and ask them to
verify a fresh artifact at 500 kbit/s on the CAN0 transceiver's CANH/CANL. They
must also confirm controller `STARTED`, transceiver `NORMAL`, and P20.6 low.

If zero traffic remains after all generated-code checks pass, stop changing the
controller configuration. Investigate, in this order: physical CAN0 connector
and termination, measurement bitrate/ACK node, TJA1040 supply and enable, then
the Tx PDU/runnable/ComM/BswM path. Do not revert the RX selector without a
schematic contradiction.

Before a commit, inspect only the task scope:

```powershell
git status --short
git diff -- Proj_Config/Bsw_Config/Cfg/Config/ECUC/TC27x_Can_ecuc.arxml Proj_Config/Mcal_Config/Cfg/config/Port.xdm
git diff -- Proj_Code/_01_BSW/Gen/GenData/Can_Lcfg.c Proj_Code/_03_MCAL/Gen/inc/Port_Cfg.h Proj_Code/_03_MCAL/Gen/src/Port_PBCfg.c
```

Stage only the confirmed ECUC/Port sources, their export/mirror files if those
were directly changed, and the direct generated outputs. Then run
`git diff --cached --check` and commit with a focused message only after the
user has confirmed the requested behavior or has authorized source-only
handoff. Preserve unrelated untracked files, IDE metadata, logs, Debug output,
and changes made by the user.

## Failure patterns

| Symptom or mistake | Correct response |
|---|---|
| `Node0` but no traffic | Check RXSEL separately: P20.7 needs `b_001` / generated value `1u`. |
| Can generation works but TX is silent | Inspect generated P20.8; it must be push-pull ALT5, not GPIO input. |
| Controller starts but transceiver remains silent | Inspect P20.6: active-low NEN/STB must be GPIO push-pull output low. |
| EB says no configuration/project | Import the `Cfg` directory, then generate project `TC275`; do not use `Cfg` as project name. |
| DaVinci edit appears to disappear | Save/close GUI before `edit_file`; its in-memory model can overwrite ARXML. |
| Wrapper says generation completed | Read the newest report; require zero errors and successful `Can` phase. |
| Need to fix generated C manually | Do not. Return to ECUC/Port.xdm, regenerate only the affected module, and sync again. |
| Diff includes Debug, logs, `.cproject`, or unrelated GenData | Leave it unstaged unless the user explicitly scopes it. |
| Build succeeded but no bus frame | A link is not a bus test; verify flash artifact, wiring, bitrate, enable, ACK, and Tx call chain. |
