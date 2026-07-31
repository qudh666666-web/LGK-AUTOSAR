# AUTOSAR ECUC Bridge

An open-source local command-line bridge for inspecting and updating AUTOSAR ECUC projects with a user-supplied, licensed Vector DaVinci Configurator installation.

The project provides local module discovery, ECUC template lookup, container lookup, scoped text edits, DaVinci validation/error listing, module generation, and normal daemon shutdown. It never bundles DaVinci, Vector SIP content, licenses, or ECUC project files.

## License and scope

This repository is MIT-licensed. It is an independent community implementation and is not affiliated with Vector Informatik GmbH. DaVinci and Vector are trademarks of their respective owners. Use requires a lawful local DaVinci installation and compliance with its license terms.

## Build

Install a current Rust toolchain, then run:

```powershell
cargo test --all-targets
cargo build --release
```

The release binaries are `autosar-ecuc-bridge` and `autosar-ecuc-bridge-host`. On Windows, keep both `.exe` files in the same directory.

Before the first GitHub push, set the `repository` field in `Cargo.toml` to the real repository URL.

## Project configuration

Create `ecuc-bridge.json` in the DaVinci Cfg directory:

```json
{
  "project_path": "D:\\Work\\Project\\Cfg",
  "tool_path": "D:\\VectorSIP"
}
```

For migration only, the reader also accepts the former `gyx-vector.json` field names. New public projects should use `ecuc-bridge.json`.

Run the executable from that Cfg directory and pass one JSON request:

```powershell
autosar-ecuc-bridge.exe '{"func":"find_module","module":"Com"}'
```

Supported functions are `find_module`, `find_module_template`, `get_param_definition`, `locate_container`, `edit_file`, `get_errors_list`, `auto_solve_errors`, `generate_code`, and `shutdown_host`. For existing automation, `find_bsw_module`, `get_bsw_module_template`, and `get_bsw_param_definition` remain accepted aliases.

`edit_file` refuses files outside `project_path`. `auto_solve_errors` requires `confirmed: true`. Always send `shutdown_host` after the final request.

## Contribution rules

- Do not contribute proprietary binaries, decompiled output, extracted scripts, credentials, licenses, customer configurations, or generated customer code.
- Implement against public documentation and locally licensed tools only.
- Add focused tests for new local operations and run `cargo test --all-targets` before opening a pull request.
- Keep DaVinci automation changes small and document the supported DaVinci version used for validation.

## Help and release policy

Read the Chinese walkthrough in `docs/使用说明.md` before the first project edit. Report reproducible defects through the GitHub issue tracker after removing customer files, credentials, license material, and proprietary Vector content. Intellectual-property reports should use the repository host's private reporting route or the repository contact published by its maintainer; a report is handled by removing or replacing the disputed material, not by retaining it until a complaint arrives.
