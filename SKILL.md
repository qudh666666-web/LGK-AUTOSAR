---
name: lgk-vector
description: Use LGK-Vector for local Vector DaVinci ECUC inspection, verified configuration edits, validation, affected-module generation, resident-host management, or maintenance of the LGK-Vector Rust source itself. Use whenever an AUTOSAR task involves a Vector .dpa project, ECUC ARXML, DaVinci-generated C/H/LSL, or this tool's source and release package.
---

# LGK-Vector

This skill is also the LGK-Vector source tree. The canonical shared installation
is `D:\Tools\LGK-Vector`. Codex discovers it through a directory junction at
`C:\Users\<user>\.codex\skills\lgk-vector`, so every AUTOSAR project reads and
updates the same source. Do not copy the tool into individual projects.

## Source and runtime discovery

Resolve the current skill directory. It must contain `Cargo.toml`, `src/`,
`scripts/`, and `tests/`; on the standard Windows setup it is a junction to
`D:\Tools\LGK-Vector`. If it is missing, use the central source path explicitly
or run `scripts/Install-LGKVectorSkill.ps1`. Do not search for or create a
project-local tool copy.

Use `scripts/Invoke-LGKVector.ps1` from the central source as the normal runtime
entry. It uses root release binaries when present, otherwise
`target/release/lgk-vector.exe` and its adjacent host. Read
`docs/跨工程接入.md` when connecting another project or when the user asks how
to install, configure, call, troubleshoot, or maintain LGK-Vector.

## Linear collaboration

When the work is tracked in the connected Linear workspace, use Linear as the
engineering record alongside the local Git history. Search or fetch the linked
issue first and use it to capture the requested ECUC scope, affected modules,
constraints, blockers, and acceptance evidence. Attach or link concise
artifacts where useful: a generation-report excerpt, verification result,
build/MAP evidence, or a release commit.

Linear is a coordination layer, not a source of configuration truth. The DPA,
ECUC ARXML, generated sources, fresh DaVinci Generation Report, and local build
remain the authoritative evidence for an AUTOSAR change. Do not let an issue
status, comment, or review substitute for the mandatory ECUC workflow or its
validation gates.

Use Linear read operations by default when a Linear issue/project is supplied
or a task is clearly tracked there. Create or update issues, comments, status,
attachments, releases, or review decisions only when the user explicitly asks
for that external update. Never create duplicate work items merely to record a
local LGK-Vector task. When writing an approved update, distinguish ECUC edits
from generated output and include the exact validation outcome; if a gate is
pending or failed, state it rather than advancing the issue as complete.

## Mandatory ECUC workflow

1. Record the target repository's Git status before generation or edits.
2. Use `find_module`, `get_param_definition`, `locate_container`, or
   `inspect_ecuc_containers` to establish the actual module, definition, and
   configured container.
3. Before `edit_file`, save and close the same project in DaVinci GUI; an open
   GUI may later overwrite external ARXML changes from its in-memory model.
   Modify ECUC only with a narrowly scoped `edit_file` request. Include an
   `expected` object with exactly the same ranges and the exact text just read;
   the tool must reject the edit if that text has changed. Do not use a general
   script or manual XML rewrite to bypass this precondition.
4. Generate only the affected module unless full generation is explicitly
   required. `generate_code` and `auto_solve_errors` must name a module;
   `module:"all"` is accepted only as an explicit opt-in.
5. Before synchronization or handoff, run `verify_delivery` against the actual
   compile-project files. Require `passed:true`; use `same_as` for exact generated
   file synchronization and `must_contain`/`must_not_contain` for task-specific
   assertions.
6. Report ECUC configuration changes separately from generated C/H/LSL output.
7. Preserve unrelated user changes and stage only task files.
8. End every resident-host session with `shutdown_host`.

## Three-minute rule for ordinary changes

Complete ordinary ECUC edits such as CAN channel, baud rate, pin, controller,
transceiver, or single-container changes within three minutes:

1. Spend at most 30 seconds on `find_module` plus read-only inspection.
2. Spend at most 60 seconds on one narrow `edit_file` request.
3. Generate only directly affected modules, with at most one generation attempt.
4. Reserve the final 30 seconds for targeted searches and `shutdown_host`.
5. At 180 seconds, stop. Report the exact blocker and ask the user to identify
   missing source or configuration instead of widening the search.

Never turn a routine edit into full-project generation. If DVCfgCmd exceeds
120 seconds, stops making progress, or approaches 2 GB memory, terminate that
session and preserve its logs. Do not repeatedly restart the same generation.

## CAN fast path and lessons learned

For a CAN0/CAN1 switch, inspect and change this chain in order:

1. Read the board schematic or pin table and record controller, RX, TX, and
   transceiver enable/standby pins.
2. Inspect `Can` for the controller node and input/output selection.
3. Inspect `CanTrcv` for the physical transceiver and DIO references.
4. Search `EcuC`, `EcuM`, `BswM`, and `Rte` for the old transceiver symbol;
   edit only files containing a confirmed stale reference.
5. Generate `CanTrcv`, then only the affected integration modules. Verify the
   active generated C/H files and build exclusions; do not run full generation.

### Validation gate after every generation

Treat an LGK `generation completed` response as provisional. Immediately read
the newest DaVinci Generation Report and require all of the following before
calling generation successful, synchronizing generated files, committing, or
reporting completion:

- Validation has zero errors.
- The requested module has `Execution Result: SUCCESS` and its `GENERATION`
  phase is successful.
- The report does not say that the generator was not started because the
  configuration contains errors.

If a driver implementation is replaced (for example TJA1043 to TJA1040),
inspect the target BSW Internal Behavior before retaining or rewriting the
`RteBswModuleInstance`. If the target provides no matching internal behavior,
timing event, or exclusive area, remove the stale RTE mapping only after
confirming its exact container and determine the supported scheduling path.
Never infer success from a wrapper response while DaVinci reports a validation
failure.

### Controller replacement checklist

For a CAN1-to-CAN0 (or inverse) replacement, treat these as one atomic change:

- Map the target network to the target `Can` controller/node and its ISR.
- Bind `CanTrcv` to the transceiver physically connected to that controller,
  including RX, TX, enable/standby DIO references; do not reuse the old
  transceiver merely because its driver still compiles.
- Replace the old transceiver implementation name in `EcuC`, `EcuM`, `BswM`,
  `CanIf`, generated callouts, build inputs, and driver include/source paths.
- Keep each Tx PDU bound to the target `CanIf` controller and target Can HW
  object; confirm the generated comments/config tables identify that network.
- Ensure startup BswM requests **Full Communication for the target main
  network**, not a secondary/inter-ECU network. CommunicationAllowed alone is
  insufficient; without the matching `ComM_RequestComMode(...FULL...)`, all
  application frames can remain silent.

### Mandatory CAN0 handoff checks

Before generating or committing a CAN1-to-CAN0 change, record the target
`Can` controller base address, `CanSRC`, RX input selection, ISR name, and the
physical transceiver part/pins from the schematic. Do not change the displayed
node name alone.

Treat the physical node and RX input multiplexer as independent settings. Never
derive `CanIOPort`/RXSEL from `Node0` or `Node1`; select the value documented for
the actual RX pin. For the TC275 CAN0 path on P20.7/P20.8, verify all of these
together: `Node0`, base `0xF0018200`, P20.7 `RXDCAN0B` with RXSEL/`CanIOPort`
`b_001`, and P20.8 push-pull alternate output 5 (`TXDCAN0`). If the TJA1040 uses
P20.6 as active-low NEN/STB, configure P20.6 as GPIO push-pull output with a low
initial level. Inspect the generated `Can_InitPortSel` and `Port_PBCfg.c`; a
correct controller with GPIO-input TX or input-mode transceiver enable produces
a silent physical bus even when generation, compilation, and flashing succeed.

For the reproducible TC275/TJA1040 conversion, read
[the TC275 CAN1-to-CAN0 five-minute runbook](references/tc275-can1-to-can0-5min.md)
before editing. It is a low-freedom procedure: use it only when the project and
schematic match the stated controller, pins, and transceiver.

After a transceiver BSWMD replacement, run one `update_project`, then inspect
the refreshed `CanTrcv` and `Rte` containers. Project Update can recreate the
target BSW Internal Behavior and an RTE event mapping while leaving its OS task,
alarm, and event references empty. Reuse the proven prior period/task only when
the target timing event has the same period; otherwise obtain the scheduling
decision instead of guessing.

Before handoff, search the active compiled RTE sources for the target
`CanTrcv_*_MainFunction()` call and for absence of the old transceiver symbol.
An error-free report without that call does not prove wake-up polling runs.
When Project Update changes `FlatExtract`, `.dpa`, DBC-derived content, or logs,
review and stage them separately; never commit those by default with the CAN
repair.

Treat `Os` and `vLinkGen` as directly affected whenever the controller ISR name
changes. Generate both after `Can`; require `Os_Isr_Lcfg.c` to define the same
`CanIsr_*` name used by `Can_Lcfg.c`, with no old ISR name left. `CanSRC` is a
CAN-driver selection, not necessarily the OS interrupt-source number: obtain
the latter from a validated matching derivative configuration, and ensure the
target ISR is assigned to an OS Application and stack before generation.

Treat `CanIf` as directly affected whenever the transceiver implementation
changes. Regenerate it and require the active `CanIf_Cfg.h` callback macro to
match the target driver (for example `CanIf_30_Tja1040_TrcvModeIndication`).
Run an incremental compile/link before handoff. If the make rules omit header
dependencies, remove only the stale, regenerable target driver object and
rebuild; never patch the driver source or generated callback macro by hand.

### CAN replacement commit and build gate

For a CAN1-to-CAN0 replacement, do **not** synchronize generated output, commit,
or report completion until one checklist record proves all of the following:

1. The affected modules have been generated: `Can`, `CanTrcv`, `CanIf`, `Os`,
   `vLinkGen`, and `Rte`.
2. Each latest DaVinci generation report has zero errors and marks its requested
   module as successful.
3. The compiled GenData is consistent: `Can_Lcfg.c` uses `Node0` and `CanIsr_0`,
   the target `CanIf` callback is present, and the target transceiver main
   function is scheduled.
4. One serialized incremental build has exited successfully; afterwards, the
   generated ELF/HEX/MAP files have fresh timestamps and the MAP contains no
   stale CAN1 or old-transceiver symbols.

If any check fails, do not commit a partial implementation. Fix the corresponding
ECUC integration module, then regenerate only that module and repeat the exact
build validation.

When launching a build outside the IDE, capture and wait for the owned
`amk`/`ctc`/`cctc` process tree to exit. Never start another build while one is
still active, and never infer success from partial console output.

When the transceiver driver package changes, treat the TASKING/CDT project
metadata as part of the atomic replacement. In `.cproject`, replace the old
driver and `mak` include paths and exclude both the old driver source directory
and its stale generated `CanTrcv_*_Cfg.c` from source discovery. Save and close
the IDE project before editing `.cproject`; an open IDE can overwrite an
external edit from its in-memory model. Regenerate the Debug makefiles from the
updated project metadata and run a serialized clean build. A successful build
against pre-existing or manually repaired Debug makefiles does not prove that
the IDE project is synchronized. Verify the new driver/config compile once,
the old driver/config compile zero times, and then inspect the linked MAP.

### Zero-traffic acceptance gate

Never declare a CAN switch fixed merely because ECUC generation or a Tasking
link succeeds. Before handing off, prove every applicable item below and label
anything that cannot be proved as pending:

1. Confirm the **actual project root** from the newest ELF/HEX path; never
   infer it from the active terminal directory or a similarly named example.
2. Confirm the linked map has the target controller ISR and target transceiver
   symbols and has no old transceiver symbol.
3. Trace one configured Tx PDU from its runnable/application call through
   `Rte_Write` or `Com_SendSignal`/`Com_MainFunctionTx`, `CanIf_Transmit`, and
   the generated `Can` Tx PDU. Do not assume that a controller change creates
   traffic if the runnable, trigger mode, or Tx task is inactive.
4. Confirm controller mode is `STARTED`, transceiver mode is `NORMAL`, and the
   selected network-management/BswM state can enable communication.
5. Record the configured bus speed, then require the tester to use that same
   speed and the physical CANH/CANL of the selected transceiver. Moving from
   CAN1 to CAN0 may require moving the cable, termination, and enable pin;
   successful programming alone proves none of these.
6. State the exact newly built HEX/ELF timestamp and require that artifact to
   be flashed. A successful build followed by flashing an older Debug artifact
   is not a valid verification.

If all software-side gates pass but no frame is observed, stop changing ECUC
blindly. Report the remaining hardware/measurement checks (CANH/CANL routing,
termination, bit rate, board supply, transceiver enable, and whether another
node ACKs) and obtain evidence before another configuration edit.

Avoid these previously observed mistakes:

- Use the ECUC short name `CanTrcv`, not a code package name such as
  `CanTrcv_30_Tja1040`. Requests accept `module`; `module_name` is a compatibility
  alias, but `module` is canonical.
- Do not assume changing `Can` also changes the transceiver. A CAN0 controller
  combined with CAN1 transceiver pins produces a silent bus.
- Do not copy an RTE mapping from another SIP. First verify that the current
  BSWMD actually defines a BSW internal behavior, timing event, and exclusive
  area. Remove obsolete mappings when it does not.
- Update EcuC initialization entries and BswM/EcuM user callouts when a driver
  implementation name changes. Search generated output for the old symbol.
  Treat preserved user-code regions such as `EcuM_Callout_Stubs.c` as source
  inputs: replace the API, channel macro, and every operation-mode macro with
  symbols verified in the target driver headers, regenerate `EcuM`, and confirm
  the user-code edit survives before synchronizing it to the compile project.
- Synchronize only outputs of modules changed for the current task. Never copy
  unrelated MCAL output from a full-generation run into the compile project.
  Before copying `Dio`, `Port`, `Mcu`, `Icu`, or `Gtm` output, prove that module
  is affected and diff it against Git; otherwise preserve the compile baseline.
  If an unrelated sync removes a business-code symbol such as a DIO channel,
  restore only those known generated files and rebuild—do not patch the
  dependent application/CDD source to hide the mismatch.
- Do not trust an old generation report or a resident DaVinci model after an
  ARXML edit. Restart once, generate once, and read the newest report timestamp.
- Do not conflate an `ELF` that links with a bus that transmits. Link success
  proves symbol resolution; it does not prove a Tx runnable, controller mode,
  physical transceiver, wiring, ACK, or the artifact that was actually flashed.
- Do not stop at `Can` and `CanTrcv` when diagnosing zero traffic. Inspect the
  Tx PDU trigger/call chain and the active BswM/NM communication state before
  modifying another module.
- Do not use modification timestamps alone as proof of deployment. Check the
  exact project root and new HEX/ELF timestamp, then verify the programming log
  identifies the same artifact.
- Do not patch generated C/H as the primary repair. Fix ECUC first; synchronize
  generated output only after a targeted generator attempt.
- Do not mix `inspect_ecuc_containers` with DaVinci-backed functions in one
  batch. Keep read-only batches separate.
- Multi-item arrays are read-only. Send `edit_file`, `auto_solve_errors`,
  `generate_code`, `update_project`, `import_dbc`, and `shutdown_host` as standalone requests so a batch can
  never leave a partially applied mutation or generation.
- Keep the central tool at `D:\Tools\LGK-Vector`; remove project-local legacy
  bridge configs, scripts, and binaries instead of maintaining two tools.

Supported functions are `inspect_ecuc_containers`, `find_module`,
`find_module_template`, `get_param_definition`, `locate_container`,
`verify_delivery`, `edit_file`, `get_errors_list`, `auto_solve_errors`, `generate_code`,
`update_project`, `import_dbc`, and `shutdown_host`. Legacy aliases for the three `find/get_bsw_*` names remain
accepted.

`verify_delivery` is a local read-only gate. Pass an absolute `root` that
contains the configured DaVinci Cfg directory, then use root-relative `path`
and optional `same_as` values. It compares exact file bytes and checks exact
required/forbidden byte strings. Enforcement defaults to true, so a missing,
stale, or invalid compiled file makes the request fail; use `enforce:false`
only to retrieve diagnostic JSON, never as acceptance evidence.

```json
{
  "func": "verify_delivery",
  "root": "D:\\Work\\Vehicle",
  "checks": [{
    "path": "Proj_Code\\_01_BSW\\Gen\\GenData\\Can_Lcfg.c",
    "same_as": "Proj_Config\\Bsw_Config\\Gen\\GenData\\Can_Lcfg.c",
    "must_contain": ["CanIsr_0", "0xF0018200u"],
    "must_not_contain": ["CanIsr_1"]
  }]
}
```

`find_module_template` is compact by default: it returns container hierarchy
and direct parameter/reference names, not every description and range. Query
the few required names with `get_param_definition`. Use `details:true` only for
explicit maintainer diagnosis. The resident Host caches the parsed template and
invalidates it when the source ARXML changes.

The executable accepts an omitted `generate_code.module` as legacy
`module:"all"` compatibility. Do not rely on that default in agent work: name
the affected module, or write `module:"all"` when full generation is genuinely
requested.

Use `update_project` to run the DPA's registered Project Update inputs. Use
`import_dbc` with an absolute `source` and a project-relative `registered_path`
when replacing a DBC already registered by the DPA. Both are standalone,
mutating requests and require the same DaVinci project to be saved and closed.
LGK-Vector snapshots the complete Cfg tree and restores it when Project Update
fails, because DaVinci may touch DPA, ECUC and System Description files before
reporting a converter error. A disk `edit_file` request first closes any
DaVinci session previously opened by the same resident Host.

`auto_solve_errors` requires a fresh error list, user approval, and
`confirmed:true`. Never treat generated C/H/LSL edits as an ECUC repair.

## Fast read-only inspection

`inspect_ecuc_containers` reads the module's ECUC ARXML without starting
DaVinci, so it remains usable while the GUI owns the `.dpa` lock:

```powershell
& "<skill-root>\scripts\Invoke-LGKVector.ps1" `
  -ProjectPath "D:\Work\Project\Cfg" `
  -Request '{"func":"inspect_ecuc_containers","module":"Com","container":"ComSignal","short_name_regex":"^MySignal$","params":["ComBitPosition"]}'
```

Multiple inspection requests may be sent as one JSON array; their matches are
returned as one flat array. Do not mix inspection and other functions in the
same batch.

## Project configuration

Place `lgk-vector.json` in the exact DaVinci Cfg directory:

```json
{
  "tool_path": "D:\\Vector\\SIP"
}
```

The project path is derived from the directory containing the JSON. If
discovery is ambiguous, also set `project_file` (relative to that directory is
preferred) and `davinci_command_path` (relative to `tool_path` or absolute).
Use `scripts/Initialize-LGKVectorProject.ps1` for a new project and require its
static doctor result before editing. Doctor resolves paths and request shape
but does not launch DaVinci or prove that generation succeeds. The legacy keys `LGK_project_path` and
`LGK_tool_path` remain accepted for existing projects.

## Maintaining LGK-Vector itself

For source changes, inspect the relevant Rust module and focused tests before
editing. Keep the tool MCU- and SIP-independent: discover DPA module files and
real `ECUC-MODULE-DEF` paths instead of hard-coding TC275, S32, Renesas,
MICROSAR, or a package layout.

After a source change:

1. Add a focused test under `src/` or `tests/`.
2. Run `cargo test --all-targets --locked`.
3. Run `cargo build --release --locked` when publishing binaries.
4. Update `CHANGELOG.md` with date, implementation commit, purpose,
   validation, and limitations.
5. Commit tool changes only in the central LGK-Vector repository.
6. In the AUTOSAR repository, commit only its own ECUC and generated outputs;
   never vendor the LGK-Vector source or stage unrelated build output.

Do not add customer DPA/ARXML/DBC files, Vector SIP content, licenses,
proprietary binaries, extracted vendor scripts, credentials, or generated
customer code to the open-source repository.
