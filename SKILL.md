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
`docs/跨工程接入.md` when connecting another project.

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
