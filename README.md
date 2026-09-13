# LGK-Vector

An open-source local command-line bridge for inspecting and updating AUTOSAR ECUC projects with a user-supplied, licensed Vector DaVinci Configurator installation.

The project provides local module discovery, read-only configured-container inspection, ECUC template lookup, container lookup, scoped text edits, DaVinci validation/error listing, module generation, and normal daemon shutdown. It never bundles DaVinci, Vector SIP content, licenses, or ECUC project files.

It is not tied to TC275 or to a particular AUTOSAR module-definition package. Module generation uses the exact `ECUC-MODULE-DEF` reference read from the selected project's ECUC configuration, such as `/MICROSAR/Com`, `/AUTOSAR/...`, or another vendor package supplied by the installed SIP.

## License and scope

This repository is MIT-licensed. It is an independent community implementation and is not affiliated with Vector Informatik GmbH. DaVinci and Vector are trademarks of their respective owners. Use requires a lawful local DaVinci installation and compliance with its license terms.

## Build

Install a current stable Rust toolchain, then run the same checks as CI:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked
& .\tests\open-source\Invoke-DependencyLicenseGuard.ps1
& .\tests\open-source\Invoke-OpenSourceGuard.ps1 -IncludeHistory
& .\tests\open-source\Invoke-PackageManifestSmoke.ps1
& .\tests\onboarding\Invoke-OnboardingSmoke.ps1
```

The release binaries are `lgk-vector` and `lgk-vector-host`. On Windows, keep both `.exe` files in the same directory. Their `--version` output includes the semantic version, Host protocol, and source build ID; the wrapper refuses a partially rebuilt or stale pair even when the semantic version alone still matches.
End users of a GitHub Release do not need Rust; Rust is required only when building or contributing from source.

## Shared Windows installation

For the shortest end-user path, download the `LGK-Vector-skill-*-windows-x64.zip` asset from a GitHub Release and extract it. It is intentionally a small Agent Skill package: a `lgk-vector` runtime folder, the two matching EXEs, two PowerShell entry scripts, concise Agent instructions, legal notices, and one user-facing `test` folder. It contains no Rust source, CI, development tests, or long maintainer documentation; using it does not require Rust. The GitHub repository remains the complete source and maintenance distribution.

On a computer without Rust or DaVinci, double-click `test\一键测试EXE.cmd` after extraction. The bundled synthetic self-test validates the two EXEs, Unicode paths, local ECUC inspection, template caching, and normal Host shutdown. It does not launch or imitate proprietary DaVinci; the target computer's lawful DaVinci/SIP installation is still required for generation and Project Update.

Keep one writable source tree for all projects, normally `D:\Tools\LGK-Vector`. Do not copy the tool into every AUTOSAR repository. Build it once, then install the Codex Skill as a directory junction to the same source:

```powershell
& "D:\Tools\LGK-Vector\scripts\Install-LGKVectorSkill.ps1"
```

The default junction is `C:\Users\<user>\.codex\skills\lgk-vector`. Every Codex task then sees the D-drive source, tests, scripts, and documentation through that link. A cloned maintenance repository also has Git history; the downloadable Release ZIP deliberately does not contain `.git` or source code. After that one-time installation, each AUTOSAR project keeps only its own `lgk-vector.json` in the DaVinci Cfg directory and calls the central wrapper:

```powershell
& "D:\Tools\LGK-Vector\scripts\Invoke-LGKVector.ps1" `
  -ProjectPath "D:\Work\Vehicle\Cfg" `
  -Request '{"func":"find_module","module":"Com"}'
```

The canonical public repository is [qudh666666-web/LGK-Vector](https://github.com/qudh666666-web/LGK-Vector). Public releases use an audited clean-root history: the private development history may contain personal author metadata or superseded product names even when the current tree is clean.

## AI-agent compatibility

The source repository uses root `AGENTS.md` and `.agents\skills\lgk-vector\SKILL.md` for maintainers. The Release ZIP uses a simple copyable skill layout: copy its complete `lgk-vector` folder into Codex, OpenCode, or Claude Code's skills directory. Its concise runtime `SKILL.md` and `AGENTS.md` then use the same PowerShell wrapper; no Codex-specific runtime is required.

## Project configuration

The safest first-time setup is one command. It creates a portable `lgk-vector.json` and immediately runs the non-mutating static doctor:

```powershell
& "D:\Tools\LGK-Vector\scripts\Initialize-LGKVectorProject.ps1" `
  -ProjectPath "D:\Work\Project\Cfg" `
  -ToolPath "D:\VectorSIP"
```

The minimal generated JSON contains only the machine-specific tool location. `project_path` is derived from the directory containing the JSON:

```json
{
  "tool_path": "D:\\VectorSIP"
}
```

If the directory contains several `.dpa` files, or the DaVinci installation contains several command executables, select them explicitly:

```json
{
  "tool_path": "D:\\VectorSIP",
  "project_file": "VehiclePlatform.dpa",
  "davinci_command_path": "D:\\Vector\\DaVinci\\Exec\\DVCfgCmd.exe"
}
```

The optional `project_file` must be a `.dpa` inside the Cfg directory and may be relative to it. A relative `davinci_command_path` is resolved from `tool_path`; an absolute path may point at a separate DaVinci installation. Without these fields, automatic discovery succeeds only when exactly one candidate exists.

Doctor verifies JSON, request shape, DPA/ECUC/BSWMD discovery, executable paths, and CLI/Host versions. It deliberately does not launch DaVinci, so `valid=true` is not proof that a licensed DaVinci session or generation can complete. Use a disposable licensed project for that final integration check.

Run the executable from that Cfg directory and pass one JSON request:

```powershell
lgk-vector.exe --start-host
lgk-vector.exe '{"func":"find_module","module":"Com"}'
```

The supplied PowerShell wrapper performs the host-start step automatically. Prefer it for automation and AI-tool integration.

Supported functions are `inspect_ecuc_containers`, `diff_ecuc`, `set_ecuc_value`, `find_module`, `find_module_template`, `get_param_definition`, `locate_container`, `verify_delivery`, `edit_file`, `get_errors_list`, `auto_solve_errors`, `generate_code`, `update_project`, `import_dbc`, and `shutdown_host`. For existing automation, `find_bsw_module`, `get_bsw_module_template`, and `get_bsw_param_definition` remain accepted aliases.

`inspect_ecuc_containers` reads saved ECUC ARXML locally and does not start DaVinci. It can therefore inspect a project while the GUI holds the `.dpa` lock. Multiple inspection requests in one array return one flat result array; do not mix inspection requests with other functions in the same batch.

`diff_ecuc` compares one module in two project-local ARXML snapshots without starting DaVinci. It reports stable semantic paths and only added, modified, or deleted parameter/reference values. Results default to 32 changes, allow at most 256, and truncate each returned value after 512 Unicode characters. Use `path_prefix` and a small `limit` to keep review output bounded.

`set_ecuc_value` changes one existing parameter or reference by module, container instance path, and definition name. It requires the exact saved `expected` value, rejects missing or ambiguous targets, reparses the candidate XML, and then uses the same checked atomic replacement as `edit_file`. It preserves unrelated bytes, including BOM, line endings, indentation, comments, and element order. Save and close the same DaVinci GUI project first.

`find_module_template` returns a compact container tree by default so large SIPs do not flood an agent context. It includes each container's direct parameter/reference names. Use `get_param_definition` for exact metadata, or pass `"details":true` only when a maintainer genuinely needs the complete flattened definition list. Resident queries cache the parsed template and invalidate that entry when the source ARXML changes.

`verify_delivery` is a read-only handoff gate for generated projects. Give it an absolute project `root` containing the configured Cfg directory and root-relative checks. Each check names the compiled `path`, may name a generated `same_as` file for exact byte comparison, and may list exact `must_contain` and `must_not_contain` strings. Enforcement defaults to true: missing, unsynchronized, or assertion-failing files make the request fail. Use `"enforce":false` only when diagnostic JSON is required.

Multi-item request arrays are read-only. Send `set_ecuc_value`, `edit_file`, `auto_solve_errors`, `generate_code`, `update_project`, `import_dbc`, and `shutdown_host` as standalone requests so a later failure cannot leave an earlier mutation half-applied.

Before `set_ecuc_value`, `edit_file`, `update_project`, or `import_dbc`, save and close the same project in the DaVinci GUI. LGK-Vector also closes its own previously opened DaVinci session before a disk edit. `import_dbc` accepts an absolute source DBC and a project-relative `registered_path`; the destination must already be registered in the DPA. It snapshots the Cfg tree, replaces that input, runs DaVinci Project Update, and reports elapsed time and logs. If conversion or update fails, DPA, ARXML, DBC, logs, and newly added project files are rolled back together while failure logs remain outside the project for diagnosis. Since v0.3.8, `generate_code` requires a module (the `module_name` alias remains supported); old callers must pass a concrete affected module or explicit `"module":"all"`. Generation automatically verifies the one fresh report in DPA `Folders/Logs` and returns `passed:true` with a compact summary only when validation and the selected generation phase pass. Missing, stale, ambiguous or unsupported reports fail acceptance without retries. `auto_solve_errors` requires an explicit module plus `confirmed: true`. Always send `shutdown_host` after the final request.

The public suite verifies the synthetic local path and failure handling. A real DaVinci generation still requires a lawful matching DaVinci/SIP installation and a disposable licensed test project; passing the public suite is strong evidence, not a claim that no defect can exist on every proprietary tool version.

## Contribution rules

- Do not contribute proprietary binaries, decompiled output, extracted scripts, credentials, licenses, customer configurations, or generated customer code.
- Implement against public documentation and locally licensed tools only.
- Add focused tests and run the complete locked public suite before opening a pull request.
- Keep DaVinci automation changes small and document the supported DaVinci version used for validation.

## Help and release policy

Read the Chinese walkthrough in `docs/使用说明.md` before the first project edit. For the shared D-drive installation and connecting another project, read `docs/跨工程接入.md`. Report reproducible defects through the GitHub issue tracker after removing customer files, credentials, license material, and proprietary Vector content. Intellectual-property reports should use the repository host's private reporting route or the repository contact published by its maintainer; a report is handled by removing or replacing the disputed material, not by retaining it until a complaint arrives.
