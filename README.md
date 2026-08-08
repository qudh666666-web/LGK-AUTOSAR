# LGK-Vector

An open-source local command-line bridge for inspecting and updating AUTOSAR ECUC projects with a user-supplied, licensed Vector DaVinci Configurator installation.

The project provides local module discovery, read-only configured-container inspection, ECUC template lookup, container lookup, scoped text edits, DaVinci validation/error listing, module generation, and normal daemon shutdown. It never bundles DaVinci, Vector SIP content, licenses, or ECUC project files.

It is not tied to TC275 or to a particular AUTOSAR module-definition package. Module generation uses the exact `ECUC-MODULE-DEF` reference read from the selected project's ECUC configuration, such as `/MICROSAR/Com`, `/AUTOSAR/...`, or another vendor package supplied by the installed SIP.

## License and scope

This repository is MIT-licensed. It is an independent community implementation and is not affiliated with Vector Informatik GmbH. DaVinci and Vector are trademarks of their respective owners. Use requires a lawful local DaVinci installation and compliance with its license terms.

## Build

Install a current Rust toolchain, then run:

```powershell
cargo test --all-targets
cargo build --release
```

The release binaries are `lgk-vector` and `lgk-vector-host`. On Windows, keep both `.exe` files in the same directory.

## Shared Windows installation

Keep one writable source tree for all projects, normally `D:\Tools\LGK-Vector`. Do not copy the tool into every AUTOSAR repository. Build it once, then install the Codex Skill as a directory junction to the same source:

```powershell
& "D:\Tools\LGK-Vector\scripts\Install-LGKVectorSkill.ps1"
```

The default junction is `C:\Users\<user>\.codex\skills\lgk-vector`. Every Codex task then sees the D-drive `SKILL.md`, Rust source, tests, scripts, and Git history through that link. Each AUTOSAR project keeps only its own `lgk-vector.json` in the DaVinci Cfg directory and calls the central wrapper:

```powershell
& "D:\Tools\LGK-Vector\scripts\Invoke-LGKVector.ps1" `
  -ProjectPath "D:\Work\Vehicle\Cfg" `
  -Request '{"func":"find_module","module":"Com"}'
```

Before the first GitHub push, set the `repository` field in `Cargo.toml` to the real repository URL.

## Project configuration

Create `lgk-vector.json` in the DaVinci Cfg directory:

```json
{
  "project_path": "D:\\Work\\Project\\Cfg",
  "tool_path": "D:\\VectorSIP"
}
```

If the directory contains several `.dpa` files, or the DaVinci installation contains several command executables, select them explicitly:

```json
{
  "project_path": "D:\\Work\\Project\\Cfg",
  "tool_path": "D:\\VectorSIP",
  "project_file": "D:\\Work\\Project\\Cfg\\VehiclePlatform.dpa",
  "davinci_command_path": "D:\\Vector\\DaVinci\\Exec\\DVCfgCmd.exe"
}
```

The optional `project_file` must be a `.dpa` file inside `project_path`. The optional `davinci_command_path` must point directly to `DVCfgCmd.exe`. Without these fields, the original automatic discovery behavior remains unchanged.

Run the executable from that Cfg directory and pass one JSON request:

```powershell
lgk-vector.exe --start-host
lgk-vector.exe '{"func":"find_module","module":"Com"}'
```

The supplied PowerShell wrapper performs the host-start step automatically. Prefer it for automation and AI-tool integration.

Supported functions are `inspect_ecuc_containers`, `find_module`, `find_module_template`, `get_param_definition`, `locate_container`, `edit_file`, `get_errors_list`, `auto_solve_errors`, `generate_code`, and `shutdown_host`. For existing automation, `find_bsw_module`, `get_bsw_module_template`, and `get_bsw_param_definition` remain accepted aliases.

`inspect_ecuc_containers` reads saved ECUC ARXML locally and does not start DaVinci. It can therefore inspect a project while the GUI holds the `.dpa` lock. Multiple inspection requests in one array return one flat result array; do not mix inspection requests with other functions in the same batch.

`edit_file` refuses files outside `project_path`. `auto_solve_errors` requires `confirmed: true`. Always send `shutdown_host` after the final request.

## Contribution rules

- Do not contribute proprietary binaries, decompiled output, extracted scripts, credentials, licenses, customer configurations, or generated customer code.
- Implement against public documentation and locally licensed tools only.
- Add focused tests for new local operations and run `cargo test --all-targets` before opening a pull request.
- Keep DaVinci automation changes small and document the supported DaVinci version used for validation.

## Help and release policy

Read the Chinese walkthrough in `docs/使用说明.md` before the first project edit. For the shared D-drive installation and connecting another project, read `docs/跨工程接入.md`. Report reproducible defects through the GitHub issue tracker after removing customer files, credentials, license material, and proprietary Vector content. Intellectual-property reports should use the repository host's private reporting route or the repository contact published by its maintainer; a report is handled by removing or replacing the disputed material, not by retaining it until a complaint arrives.
