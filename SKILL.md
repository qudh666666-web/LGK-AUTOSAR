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

## Mandatory ECUC workflow

1. Record the target repository's Git status before generation or edits.
2. Use `find_module`, `get_param_definition`, `locate_container`, or
   `inspect_ecuc_containers` to establish the actual module, definition, and
   configured container.
3. Modify ECUC only with a narrowly scoped `edit_file` request. Do not use a
   general script or manual XML rewrite to bypass the tool.
4. Generate only the affected module unless full generation is explicitly
   required.
5. Report ECUC configuration changes separately from generated C/H/LSL output.
6. Preserve unrelated user changes and stage only task files.
7. End every resident-host session with `shutdown_host`.

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
- One invalid batch item can abort the batch. Validate module names separately
  before batching mutations or generation.
- Keep the central tool at `D:\Tools\LGK-Vector`; remove project-local legacy
  `gyx-vector` configs, scripts, and binaries instead of maintaining two tools.

Supported functions are `inspect_ecuc_containers`, `find_module`,
`find_module_template`, `get_param_definition`, `locate_container`,
`edit_file`, `get_errors_list`, `auto_solve_errors`, `generate_code`, and
`shutdown_host`. Legacy aliases for the three `find/get_bsw_*` names remain
accepted.

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
  "project_path": "D:\\Work\\Project\\Cfg",
  "tool_path": "D:\\Vector\\SIP"
}
```

If discovery is ambiguous, also set absolute `project_file` and
`davinci_command_path`. The legacy keys `LGK_project_path` and `LGK_tool_path`
are accepted for existing projects, but new projects use the lowercase keys.

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
