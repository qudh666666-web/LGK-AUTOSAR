# LGK-AUTOSAR 更新记录

版本号说明发布顺序，Git 提交号用于定位准确源码。每次功能、接口、包装器或 Skill 改动，都必须在顶部新增记录。

## 2026-10-07 - 本地待发布

- 新增正式 `write_asw_bundle`：在工程内 DPA 已登记的应用输入目录，创建/更新/删除专用 typed ASW 文件，支持四种 Implementation Type、SR/CS Interface、Application SWC、P/R Port、Runnable/Timing Event/数据访问及 Composition 的 Assembly/Delegation。完整旧字节、预览、无覆盖创建、引用/DEST/类型环/连接兼容性检查；阻止外部引用对象被删改，工程外或不可访问的应用输入使写入拒绝。查询与已有 ASW 引用修改覆盖工程内登记输入。写入保存的 ARXML，并非原生内存创建；跨请求撤销尚未实现。维护者隔离样本在真实 DaVinci 5 中完成三阶段重载/导出/保存：创建和更新各确认 15 个关键对象及类型，周期由 0.01 改为 0.02 秒，删除后包消失；每阶段约 18 秒，ERROR 计数均 254，506 个源文件和原 INI 不变。未验收 RTE 生成、Data–Signal/RTE–OS 映射或完整 ASW 类型。离线指定脚本重建双 EXE 均报 0.4.2；80 项 Rust 通过、1 项忽略，83 项 onboarding、17 项包检查及格式/Clippy/许可证/当前内容守卫通过。公开测试为合成测试，许可验收材料在仓库外；详见 `docs/ASW写入与验收.md`。

- 新增维护者专用 DaVinci 5 单 Boolean 原生实验（不进入运行时/发布包）：仅在新的非客户副本中核对旧值，事务修改、模型重查、保存并恢复，记录校验计数及复制文件变化。首次加载因内存保护中止；查明原启动配置 Xmx 为 16 GiB，改用独立 1 GiB 配置后通过，未修改 SIP INI 或放宽保护。最终真实实验约 19 秒，目标文件原字节恢复，254 个既有 ERROR 的计数前/写后/恢复后相同；506 个源文件保持不变，但副本 DPA/Os 文件出现加载/保存副作用，不能称整工程撤销成功。默认写入路径不变。新增 PrepareOnly 隔离/路径/堆参数合成检查及实验脚本包排除检查；离线重建双 EXE 均报 0.4.2，74 项 Rust 测试通过、1 项忽略，75 项 onboarding、15 项包检查及格式/Clippy/许可证/当前内容守卫通过。原生接口依据与撤销协议见 `docs/原生写入实验与安全撤销设计.md`；持久记录、旧/新值与整文件哈希复核、外部修改拒绝和中断恢复仍是设计，跨请求撤销 API 尚未实现。

- 新增只读 `inspect_autosar_mapping`：保存的 assembly/delegation、Data 到 Signal/Group、SWC/BSW event-to-task 条目汇总与定向分页；默认 8 条/最大 32 条/64 KiB，支持类别、路径、待核查项和纯汇总。保留匿名条目的 XML 位置、引用分组位置及组件实例上下文；缺 task-ref 和范围外引用不当成 DaVinci 错误。修复引用遍历误把 delegation 的结构性 `*-REF` 包装器当叶子的漏查，新增反例测试。指定脚本离线重建，CLI/Host 均报 0.4.2；74 项 Rust 测试通过、1 项忽略，65 项 onboarding、13 项包检查及格式/Clippy/许可证/当前内容守卫通过。实际工程只读核对 68 条 Port、18 条 Data、136 条任务映射（119 SWC/17 BSW），91 条缺 task-ref；汇总约 0.8 KiB，87 个配置文件哈希及客户 Git 状态未变化。没有运行 DaVinci，不声称完整连接覆盖、周期/优先级校验或写入验收。

- `inspect_ecuc_containers` 增加默认 32 项/最大 256 项和 64 KiB 响应预算。小结果保留数组；超过匹配上限的旧请求返回 `INSPECTION_PAGE_REQUIRED`，使用 `paged:true` 返回总数、分页结果、完整性与后续 offset。拒绝非法分页参数及超大值输出，不静默丢失条目/截断值；新增合成分页、末页、非法输入、超大字段及包装器检查。指定脚本离线重建成功，双 EXE 均报 0.4.2；70 项 Rust 测试通过、1 项可选检查忽略，63 项 onboarding、13 项包检查及格式/Clippy/许可证/当前公开内容守卫通过。未运行真实 DaVinci，未修改客户工程。

- 基于 AutoC 0.10.0 安装包与公开说明的差距审查，新增只读 `audit_autosar_model`：在现有有界 System/Developer 索引上统计 `*-REF` / `*-TREF` 的 `unresolved_in_scope`，按角色汇总，默认仅回传 8 个例子（最多 32 个）。可按源对象类型/路径缩小结果；不把扫描范围外引用误报为已证实的模型错误。审查结论见 `docs/AutoC-0.10.0-能力差距审查.md`。指定脚本离线重建成功，双 EXE 均报 0.4.2；68 项 Rust 测试通过、1 项可选检查忽略，62 项 onboarding、13 项包检查和格式/Clippy/许可证/公开内容守卫通过；没有运行 AutoC 或客户 DaVinci 工程。
- ASW 引用追踪现识别 AUTOSAR `*-TREF`，可沿 Port → Interface → Data Type 等已有关系定向排查。新增独立请求 `set_asw_reference`：只允许修改当前工程 `Config/Developer` 下已有对象的一个 `*-REF` / `*-TREF`，要求准确旧目标、唯一对象/角色、新目标存在且匹配 `DEST`，支持预览并通过原文件复核与原子替换提交。缺失目标、类型不符、越界文件和批量写入均拒绝。该功能不创建 SWC/Port/Type/Mapping，也不替代 DaVinci 校验。指定脚本离线重建后双 EXE 均为 0.4.2；67 项 Rust 测试通过、1 项可选检查忽略，62 项 onboarding、13 项包检查及格式/Clippy/许可证/公开内容守卫通过。
- `inspect_autosar_model` 新增 `summary_only:true`：本地扫描 System/Developer ARXML，按已有过滤条件返回对象总数和最多 64 种 AUTOSAR 元素的计数，不返回逐条对象或引用。适用于大型 DBC 导入后的低 token 初筛；不表示新旧 DBC 差异或映射正确性。
- 新增 10,000 个合成信号的有界输出测试：单次汇总 JSON 小于 1 KiB。指定脚本离线构建成功，CLI/Host 均为 0.4.2；65 项 Rust 测试通过、1 项可选检查忽略，62 项 onboarding、13 项包清单检查通过；格式、Clippy `-D warnings`、公开内容与依赖许可证检查通过。不启动真实 DaVinci、不修改客户工程，真实 DBC 导入未在本轮重跑。

## v0.4.2 - 2026-09-27

- Generation Report validation failures now include bounded error codes, messages and affected paths when available, instead of only totals.
- Added standalone `set_ecuc_values` for up to 32 existing ECUC values: preview, full preflight, original-byte recheck, and best-effort rollback on later write failure. Cross-file writes are not crash-atomic.
- Verified generation now returns content-changed GenData files and, with optional `delivery`, which changed files need synchronization. This is advisory; `verify_delivery` remains the exact-byte handoff gate.
- Synthetic Rust tests cover report details, grouped preflight/preview/rollback and generated-file diff. Local verification: 64 Rust tests passed (one optional test ignored), 62 onboarding checks, 13 package checks, dependency-license and public-content guards passed. Both release EXEs report 0.4.2. Existing project-private rustfmt and Clippy binaries were found and used; two pre-existing formatting deviations were corrected for CI. Windows CI exposed short-path fixture mismatches, corrected by canonicalizing synthetic test paths. No system components were installed, and no customer ECUC or generated file was changed.
- Public history guard now inspects ancestors of the published HEAD, not every fetched sibling branch; a clean-root branch can pass CI with `fetch-depth: 0` while an old branch containing private history still fails its own check.

## 2026-09-26 DaVinci startup budget

- A large TC275 project remained in normal DaVinci project loading after the previous 45-second startup limit; the bridge stopped the process before generation could begin.
- Raised daemon startup wait to 120 seconds, Host request wait to 270 seconds, and PowerShell request wait to 280 seconds. Kept the 120-second operation limit and single-module generation/report gate.
- Verification: local release build succeeded; CLI and Host both report `0.4.1 protocol=2 build=dev`; 60 Rust tests passed and one optional test was ignored. The PowerShell wrapper parsed without errors. Clippy could not run because the private Rust toolchain lacks that component; no system installation was attempted. The affected TC275 project reached DaVinci generation: `Can`, `CanIf`, `CanSM`, and `CanTrcv` each passed a fresh report with zero validation errors. No ECUC files are stored in this repository.

## 2026-09-14 Legacy entry deprecation

- Added README/AGENTS/SKILL migration notices in the local legacy directory.
- Legacy PowerShell requests now fail with the current LGK-AUTOSAR entry path;
  standalone shutdown_host remains allowed for cleanup. Direct legacy EXEs
  are unchanged and do not read this notice.
- Syntax and rejection-before-project-access checks passed. Versioned copies
  are kept in assets/legacy-deprecation; no customer configuration was changed.

## v0.4.1 - 2026-09-14

- Implementation commit: `cbfebec`. Borrowed upfront field validation from
  claude-autosar and independently implemented a compact locator contract.
  `--describe locate_container` works without project configuration or Host.
- Locator requests report missing/invalid/unknown fields together with an
  example. Unsupported query/path now fail instead of being silently ignored.
- Results default to 32 containers, maximum 256, with offset/next_offset.
  count retains the full match count; callers must follow truncated pages.
- Verified: 60 Rust tests passed, 1 optional private test ignored; release pair
  built as 0.4.1; Clippy, changed-file formatting, 62 onboarding checks,
  13 package-manifest checks and public-content guard passed.
- Scope: locator only; no new dependencies, customer configuration edits or
  DaVinci generation. Token savings are not measured. Cloud release unchanged.

## v0.4.0 - 2026-09-13

- Implementation commit: `a4b187b`. Renamed the complete product from
  LGK-Vector to LGK-AUTOSAR: Cargo package/library, CLI and Host binaries,
  DaVinci adapter task, PowerShell entry points, Skill identity and folder,
  project configuration, pair manifest, documentation and release packaging.
- New projects use `lgk-autosar.json`; the runtime still reads an existing
  `lgk-vector.json` as a migration fallback. New binaries and scripts use only
  the LGK-AUTOSAR names, so mixed old/new executable pairs are rejected.
- Version moved to 0.4.0 because executable, script, crate, Skill and package
  names changed. The GitHub repository and cloud release assets are renamed in
  the same release.
- Verified: 58 Rust tests passed and one optional private-report test was
  ignored; release CLI/Host both report 0.4.0; Clippy `-D warnings`, public
  content guard, package manifest (13 assertions) and onboarding (62 checks)
  passed before publication.

## v0.3.12 - 2026-09-13

- Implementation commit: `7260a3a`. Added read-only `inspect_autosar_model`
  and `trace_autosar_model` through doctor, resident Host, CLI and PowerShell
  wrapper. They index generic AUTOSAR objects by XML kind, `SHORT-NAME`, stable
  semantic path, source file and `*-REF` relationships across saved System and
  Developer ARXML, without hard-coding Vector module display names.
- Trace supports incoming, outgoing and bidirectional traversal. Named
  containment edges preserve Frame-to-mapping and PDU-to-signal-mapping links;
  AR-PACKAGE containment is omitted to reduce noise. Repeated definitions in
  Communication and SystemExtract resolve as one semantic start while all
  matching references remain traversable.
- Query output defaults to 32 objects and trace defaults to depth 3, 64 nodes
  and 128 edges. Hard limits are 512 ARXML files, 64 MiB per file, 256 MiB total,
  depth 6, 256 nodes and 512 edges. Symlinks, oversized scopes, invalid regular
  expressions and genuinely ambiguous start paths fail without starting
  DaVinci or modifying the project.
- Verified: release build produced matching v0.3.12 CLI/Host; 57 Rust tests
  passed and one optional private-report test was ignored; Clippy `-D warnings`,
  changed-file rustfmt and diff checks passed. A read-only scan of the current
  Vector project indexed 2,559 objects from 20 model ARXML files (2.26 MB) in
  about 2.4 seconds, and traced a real CAN Frame through its mapping to an N-PDU.
- Limits: this release provides generic discovery and dependency tracing. It
  does not yet create SWCs, ports, interfaces, data types or data mappings, and
  it does not claim that a DBC Project Update created every application artifact.
  No customer file was copied into the repository or changed during validation.

## v0.3.11 - 2026-09-13

- Implementation commit: `ebb2852`. Added `set_ecuc_value` to doctor, resident
  Host, CLI, PowerShell wrapper, source/release Skills and onboarding/package
  self-tests. Existing parameter values and references can now be changed by
  real DPA module, container instance path, definition name, exact saved
  `expected`, and new `value`; no XML lines are required in the request.
- The operation requires one direct, unique semantic target. It compares a full
  parsed XML-tree result with the byte-preserving raw location, escapes the new
  text, reparses and semantically re-locates the candidate, then rechecks the
  original bytes and uses the existing same-directory atomic replacement.
  Commented-out XML cannot become a write target. BOM, line endings, indentation,
  comments, attributes, ordering and every unrelated byte remain unchanged.
- Missing containers/values, duplicate paths/definitions and stale expected
  values fail without writing and have stable ECUC_CONTAINER_NOT_FOUND,
  ECUC_VALUE_NOT_FOUND, ECUC_VALUE_AMBIGUOUS or
  ECUC_VALUE_PRECONDITION_FAILED diagnostics. New values are limited to 4096
  Unicode characters; returned old/new values are capped at 512 and only emit
  truncation flags when applicable. Mutating batch and open-owned-GUI protections
  are the same as `edit_file`.
- Verified: release build produced matching v0.3.11 CLI/Host; 52 Rust tests
  passed and one optional private-report test was ignored; Clippy `-D warnings`,
  changed-file rustfmt and diff checks passed; the final onboarding suite passed
  62 assertions twice; packaged EXE self-test passed 14 assertions; package
  manifest passed 13; current-content and dependency-license guards passed.
- Limits: changes only an existing direct parameter `<VALUE>` or reference
  `<VALUE-REF>`. Creation/deletion, container structure and self-closing empty
  values still require a narrow `edit_file`. No live DaVinci operation, customer
  project mutation, public push/tag/Release or public snapshot sync was run.

## v0.3.10 - 2026-09-13

- Implementation commit: `53bd44d`. Borrowed the stable path plus
  add/modify/delete result shape from claude-autosar and independently
  implemented the Rust ECUC-specific parser, safety limits and tests. Source
  links and adoption boundaries are recorded in docs/开源借鉴记录.md; no
  upstream code or new runtime dependency was copied.
- Added read-only `diff_ecuc` through doctor, resident Host, CLI and PowerShell
  wrapper. It compares one concrete module in two project-contained ARXML files
  and reports only changed parameter/reference values at stable semantic paths.
  Inputs may be project-relative or absolute contained paths; files outside the
  project, non-ARXML inputs and inputs over 64 MiB are rejected.
- Output defaults to 32 changes, has a hard 256-change limit and caps each
  returned old/new value at 512 Unicode characters with per-value truncation
  flags. `path_prefix` narrows results. Counts describe the complete filtered
  result before truncation; duplicate semantic paths fail with
  ECUC_DIFF_AMBIGUOUS_PATH rather than being paired heuristically.
- Verified: release build produced matching v0.3.10 CLI/Host; 45 Rust tests
  passed and one optional private-report test was ignored; Clippy `-D warnings`,
  changed-file rustfmt and diff checks passed; onboarding passed 59 assertions;
  packaged EXE self-test passed 12 assertions; package manifest passed 13;
  current-content and dependency-license guards passed.
- Limits: compares saved ECUC parameter/reference values only. Container
  metadata, UUID, comments, ordering and formatting changes are omitted;
  container renames appear as delete plus add. No live DaVinci operation,
  customer project mutation, LLM-token benchmark, public push/tag/Release or
  public snapshot sync was performed.

## v0.3.9 - 2026-09-13

- Implementation commit: `8fe352d`. Borrowed the structured-field diagnostic
  idea from claude-autosar, independently implemented in Rust with no copied
  source or new dependency. Sources and adoption decisions: docs/开源借鉴记录.md.
- Strict verify_delivery failures now include compact JSON with stable code,
  original zero-based check_index, missing files, synchronization and required/
  forbidden pattern failures. No enforce:false rerun is needed for the usual
  diagnostic. Limits: 8 failed checks, 8 patterns per kind, counts and truncation
  indicators. Passing checks are omitted from the strict failure response.
- Missing/unknown modules return MODULE_REQUIRED / MODULE_NOT_FOUND. Unknown
  names include up to 8 real DPA candidates; suggestions never silently select
  a module for generation. Case-insensitive matching and module_name remain.
- Existing nonzero exits and CLI/PowerShell exception envelopes are retained;
  JSON is in the error detail, not successful stdout. Other errors remain text.
  Normal successful responses and dependency versions are unchanged.
- Verified: 43 Rust tests passed (one optional private-report test skipped),
  Clippy -D warnings, changed-file formatting and git diff checks passed;
  Build-LGKAutosar.ps1 rebuilt local v0.3.9 CLI/Host; 56 onboarding assertions
  prove JSON details survive the Host/CLI/wrapper and failures still fail;
  13 package assertions and current-content guard passed.
- Limits: no live DaVinci generation or LLM-token benchmark was performed.
  No customer project, public snapshot, GitHub push/tag/Release was changed.

## v0.3.8 - 2026-09-13

- Implementation commit: `4e1e39a`.
- Generation now checks the single new/rewritten GenerationReport.html in
  DPA Folders/Logs before returning passed:true. It requires zero fatal/errors,
  successful selected generator and GENERATION phase, matching the actual ECUC
  definition rather than a display/package name. SUCCESSFUL and WARNING report
  summaries are supported, but do not waive the selected-generator checks.
  Discovery only reads metadata within two log-directory levels (8192 entries);
  only the changed report is read, capped at 32 MiB. Missing/stale/ambiguous or
  unknown report formats fail acceptance without retrying generation.
- generate_code requires module or the existing module_name alias. Full
  generation remains explicit module:"all". Existing callers that omitted
  module must migrate; optional-module queries are unchanged.
- edit_file writes/syncs a unique same-directory temporary file, rechecks the
  original bytes and uses rename replacement with no copy-over fallback.
  This is not an external-writer lock or a power-loss durability guarantee;
  per-file ACLs, alternate streams and hard-link identity are not preserved.
- Verification: 40 Rust tests passed (one optional private-report test ignored
  by default); that private check was separately run on two existing local
  reports and correctly accepted success/rejected validation error. Debug
  parser timings were approximately 10/27 ms, excluding generation and report
  discovery. Clippy -D warnings and rustfmt checks passed using already installed
  private components. Release build passed via Build-LGKAutosar.ps1; onboarding
  51 assertions, package manifest 13 assertions and current-content guard passed.
- Limits: no live DaVinci generation was run and no customer project was changed.
  Report compatibility is based on the locally inspected HTML layout and
  synthetic regression cases, not every DaVinci version. Local binaries are
  v0.3.8 build=dev; no public snapshot sync, push, tag or Release was performed.

## v0.3.7 - 2026-08-29

- Core: the resident Host is now spawned through `CreateProcessW` with
  `bInheritHandles=FALSE`, `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`, and
  explicit empty standard streams. Rust's `Command` spawn cannot disable
  handle inheritance, so every inheritable pipe in the caller chain
  (PowerShell capture pipes, bash/MSYS pipelines, .NET redirection pipes)
  also entered the never-exiting Host, and the caller's pipeline then stayed
  open forever after all foreground processes had exited. This reproduced as
  a silent indefinite hang with zero output when the wrapper ran from a
  backgrounded Git Bash pipeline on a real TC275 project (2026-08-29). The
  v0.3.0 mitigation only cleared the CLI's own three standard handles and
  could not cover these outer inheritance sources. Implementation commit:
  `530a744`.
- Wrapper: every CLI invocation in `Invoke-LGKAutosar.ps1` now runs under a
  hard watchdog (`--version` 15s, `--start-host` 60s, doctor 90s, request
  185s). On timeout the child process tree is terminated and the captured
  partial output is reported, so a wedged CLI can no longer hang the
  caller's shell or an agent pipeline indefinitely. The wrapper now invokes
  the CLI through .NET redirection with an explicit working directory and
  still exports the CLI exit code through `$LASTEXITCODE` for callers such
  as the initializer; output and error message formats are unchanged.
- Tests: the onboarding suite gained a pipe-topology regression probe. It
  runs the first real wrapper request through a redirected child PowerShell
  and requires both process exit and stdout EOF within a budget; the
  pre-fix binary fails this probe while the fixed binary completes in
  seconds. The Rust suite gained `CreateProcessW` command-line quoting tests.
- Tooling: maintainer scripts are now Windows PowerShell 5.1 safe on Chinese
  Windows. `Build-LGKAutosar.ps1` and the onboarding suite carry a UTF-8 BOM
  so their Chinese text no longer breaks parsing or degrades into mojibake
  fixture paths; the onboarding temporary fixture therefore exercises real
  Chinese-and-space paths again. `Sync-LGKAutosarPackage.ps1` and
  `Invoke-PackageManifestSmoke.ps1` now read Git path output with a UTF-8
  console encoding (restored afterwards), because `git ls-files` emits raw
  UTF-8 bytes that PS 5.1 otherwise decodes as ANSI, silently dropping
  Chinese-named files from manifests and fixtures.
- Validation: 33 Rust tests passed (31 previous plus 2 new); the onboarding
  suite passed 50 assertions including the pipe regression probe and the
  packaged-binary path; the dependency-license guard and the package
  manifest smoke (13 assertions) passed on the local Chinese-Windows
  PowerShell 5.1 machine; `Build-LGKAutosar.ps1` ran end to end under
  PowerShell 5.1. The exact incident topology (backgrounded bash pipeline
  plus a fresh Host spawn) completed in 5 seconds on the fixed build versus
  an indefinite hang before, and a real TC275 read-only request followed by
  `shutdown_host` then verified end to end with both ports released.
- Build: release CLI and Host both report `0.3.7`, protocol 2, rebuilt with
  the project-private offline GNU toolchain. The private toolchain still
  lacks rustfmt/clippy, so formatting was reviewed manually; compilation and
  all executable tests passed.
- Limitations: `Invoke-OpenSourceGuard.ps1 -IncludeHistory` still rejects the
  private development history, as documented since v0.3.0.

## v0.3.6 - 2026-08-16

- Core: add the generic read-only `verify_delivery` request. It checks actual
  compiled files against generated sources with exact byte comparison, enforces
  required/forbidden text assertions, and fails closed by default when a file
  is missing, stale, or invalid. Implementation commit: `438fef9`.
- Safety: require one declared absolute project root that contains the configured
  DaVinci Cfg directory; accept only root-relative file paths, reject `..`, drive
  prefixes, and link escapes, limit checks/patterns/file size, and never return
  inspected file contents.
- Validation: 31 Rust tests passed; the onboarding suite passed 48 assertions;
  the packaged EXE suite passed 11 assertions; public-content, 13-item package,
  and 34-dependency license guards passed. A real TC275 read-only request proved
  both `Can_Lcfg.c` and `Port_PBCfg.c` synchronized into the compile project and
  confirmed the required CAN0 symbols, then `shutdown_host` released the Host.
- Build: release CLI and Host both report `0.3.6`, protocol 2, and were rebuilt
  with the project-private offline GNU toolchain. The private toolchain lacks
  the optional `rustfmt` component, so format validation was manual; compilation
  and all executable tests passed. No public tag or GitHub release was created.

- Skill: add a detailed, low-freedom five-minute runbook for the proven TC275
  CAN1-to-CAN0/TJA1040 conversion. It separates DaVinci `Can` ECUC edits from
  EB tresos `Port.xdm` generation, records the actual P20.6/P20.7/P20.8 mapping,
  synchronizes only compiled outputs, and requires generated-value assertions
  before handoff or Git commit.
- Validation: the runbook is based on the repaired TC275 configuration where
  `Can_InitPortSel=1`, P20.6 is GPIO output-low, P20.8 is ALT5 output, and the
  user confirmed normal bus traffic after generation and synchronization.

- Skill: make the physical pin mux and receive selector mandatory in a CAN0
  handoff. It now forbids deriving RXSEL/`CanIOPort` from the node number and
  records the proven TC275 CAN0 mapping: Node0/base `0xF0018200`, P20.7
  `RXDCAN0B` with `b_001`, P20.8 `TXDCAN0` on push-pull ALT5, and active-low
  TJA1040 NEN/STB on P20.6 as GPIO output low.
- Validation: the compiled `Port_PBCfg.c` had P20.6 and P20.8 as GPIO inputs and
  `Can_InitPortSel` was 0, which produced a completely silent bus despite a
  successful build. Targeted Port and Can generation produced P20.6 output,
  P20.8 ALT5, and `Can_InitPortSel=1`; both generators reported zero errors.

- Skill: add a non-bypassable CAN0 commit/build gate after an incomplete CAN0
  handoff. It requires successful Can/CanTrcv/CanIf/Os/vLinkGen/Rte generation
  evidence plus one serialized incremental build and fresh ELF/HEX/MAP
  verification before synchronization, commit, or completion reporting. It
  explicitly forbids concurrent/restarted builds and interpreting partial build
  output as success.
- Validation: the real TC275 sequence initially exposed a missing `CanIsr_0`
  definition and a stale TJA1040 CanIf callback after premature handoff. After
  correcting the Os/vLinkGen/CanIf generation and synchronization, a single
  rebuild produced fresh ELF/HEX/MAP files with `CanIsr_0` and TJA1040 symbols
  and without `CanIsr_1` or TJA1043 symbols.

- Skill: add the required OS/vLinkGen and CanIf portions of a controller or
  transceiver replacement. The handoff now proves the ISR name is consistently
  generated in Can and Os, distinguishes `CanSRC` from the derivative-specific
  OS interrupt-source number, verifies OS-Application/stack assignment, and
  requires regeneration of the transceiver callback macro plus a real link.
- Validation: applied to the TC275 CAN0 repair after a build exposed an
  undefined `CanIsr_0` and then an unresolved
  `CanIf_30_Tja1040_TrcvModeIndication`; Os, vLinkGen, and CanIf were generated
  with zero report errors, the stale driver object was rebuilt, and the final
  MAP contained TJA1040/CanIsr_0 with no TJA1043/CanIsr_1 symbols.

- Skill: make the CAN1-to-CAN0 checklist executable at handoff. It now requires
  recording `CanSRC` alongside controller/pin data, running Project Update once
  after a transceiver BSWMD replacement, checking the regenerated RTE task
  mapping, and proving the compiled RTE invokes the target transceiver main
  function. It also requires separating Project Update side effects from the
  CAN commit.
- Validation: applied to a TC275 CAN1-to-CAN0/TJA1043-to-TJA1040 repair:
  `CanSRC=1` was rejected for Node0; Project Update created the target behavior
  but initially left its 5 ms RTE mapping incomplete; regenerated Can, CanTrcv,
  and Rte reports each had zero errors after the target task binding was added.

- Skill: generation responses are now provisional until the newest DaVinci Generation Report proves zero validation errors and a successful `GENERATION` phase; driver replacements must verify target BSW Internal Behavior before retaining RTE BSW-instance mappings.
- Validation: reviewed against a real TJA1043-to-TJA1040 replacement where DaVinci reported `RTE01006` although the wrapper returned a completion response; skill syntax validation is pending because the bundled validator Python lacks `PyYAML`.

- 新增 `scripts/Build-LGKAutosar.ps1` 作为中央源码仓库唯一的 EXE 构建入口：在当前进程绑定工程内私有 Rust、Rustup 与已验证的 `dlltool.exe`，默认离线构建，不改系统环境、DaVinci 或 AUTOSAR 工程；
- 实现提交：本次变更提交（提交后以 Git 日志中的 `Reject mutating request batches in wrapper` 定位）；
- 包装器在启动 Host 前拒绝多项数组中的 `edit_file`、`auto_solve_errors`、`generate_code`、`update_project`、`import_dbc` 与 `shutdown_host`，使其与既有 Rust 调度约束一致；
- 将 `Set-StrictMode` 与 `$ErrorActionPreference` 前移至参数声明之后；错误输出增加退出码与请求文件路径；白名单注释指向真实 Rust 注册位置；源码 Junction 安装器补充 Codex、Claude Code、OpenCode 的常见路径；发行包守卫拒绝 `.pdb`；
- 验证：5 个受改动影响的 PowerShell 文件通过语法解析；`Invoke-PackageManifestSmoke.ps1` 13 项断言、`Invoke-OnboardingSmoke.ps1` 46 项断言、从新建 Release 目录运行的 `Run-ExeSelfTest.ps1` 11 项断言均通过；常规公开内容守卫通过，Host 端口已释放；
- 限制：当前命令环境未发现 `cargo`，故未运行 Rust 格式、Clippy、测试、构建及依赖许可证守卫；中央维护仓库保留私有历史，`-IncludeHistory` 守卫按设计失败。本次不发布新二进制、不创建标签，公开版本仍为 `v0.3.5`。

## v0.3.5 - 2026-08-15

- 将 Release 包自检脚本和双击入口的用户可见提示全部改为中文；许可证与第三方声明保持原文不改；
- 验证：从 ZIP 中 `test/一键测试EXE.cmd` 对应的 PowerShell 自检路径运行成功。

## v0.3.4 - 2026-08-15

- Release 技能包中的 `README.md`、`SKILL.md` 和 `AGENTS.md` 全部改为中文；源码仓库仍保留面向开源维护者的英文说明；
- 验证：重新打包后从中文说明包的 `test/` 目录运行 EXE 自检。

## v0.3.3 - 2026-08-15

- GitHub Release 改为极简 `LGK-AUTOSAR-skill` 包：根目录仅保留安装说明、许可证、`lgk-autosar/` 运行时和 `test/`；不再分发 Rust 源码、CI、开发测试、维护文档或 Git 元数据；
- 运行时目录采用可复制技能结构，内含 CLI/Host、PowerShell 包装器、初始化器、简版 `SKILL.md` 和 `AGENTS.md`；README 明确列出 Codex、OpenCode、Claude Code 的项目级与全局安装位置；
- 自检和 onboarding 改为从 `lgk-autosar/` 启动实际 EXE/Host，打包守卫断言源码、长文档和 CI 不会进入 Release。

## v0.3.2 - 2026-08-15

- 源码根目录收敛为单一 `tests/`；将发行版 EXE 自检移至 `tests/release/`，打包时自动映射为 ZIP 中唯一面向用户的 `test/`，不再把开发/CI 测试带入发行包；
- 新增根目录 `AGENTS.md` 作为跨 Agent 入口：Codex 可继续使用 `SKILL.md`，OpenCode 和其他遵循 `AGENTS.md` 的 Agent 使用相同请求、验证和正常关闭流程；
- 验证：打包守卫断言发行包存在 `test/Run-ExeSelfTest.ps1`、不存在 `tests/`，onboarding 从生成包实际调用 CLI/Host。

## v0.3.1 - 2026-08-15

- 修复 Windows PowerShell 在部分 `-File` 启动路径下将 `$PSScriptRoot` 置空时，发布包“一键测试 EXE”、包装器、初始化器和打包脚本无法定位自身目录的问题；现统一回退到 `$MyInvocation.MyCommand.Path`，不可识别时给出明确错误；
- 发布包自检继续保留在发行包的 `test` 目录；源码的 `tests` 仍只承担 Rust、onboarding、打包与开源合规检查；
- 验证：从新打包目录以 `powershell.exe -NoProfile -ExecutionPolicy Bypass -File` 运行自检，11 项断言全部通过。

## Unreleased - 2026-08-12

- 固定公开仓库地址为 `https://github.com/qudh666666-web/LGK-AUTOSAR`；发布时只推送经过审计的 clean-root 公共快照，不公开含个人邮箱和旧名称的私有开发历史；
- 对齐既有 Vector 自动化入口的 Windows 行为：包装器固定使用 UTF-8 输入、输出和无 BOM 请求编码，中文、空格路径加入端到端回归；
- 修复 Windows `connect_timeout` 后套接字偶发保留非阻塞状态，导致 Host 探测把 `10035/WouldBlock` 误报为端口冲突的问题；
- `find_module_template` 默认改为轻量容器树，只返回容器层级及参数/引用名称；完整描述、范围和目标仅在 `details:true` 时返回，日常精确查询继续使用 `get_param_definition`；
- 为 SIP 模板建立按工具路径和真实 definition ref 索引的 resident 缓存，并用文件长度和修改时间失效，避免每次查询重复扫描整个 SIP；
- 保留旧调用兼容性：`generate_code` 省略 `module` 时等价于 `module:"all"`；Skill 的日常流程仍必须显式指定受影响模块，避免无意全量生成；
- 发布包新增 `test` 目录和双击式 EXE 自检；没有 Rust/DaVinci 的电脑也能验证 CLI/Host 配对、中文路径、本地 ECUC 查询、模板缓存和正常关闭，自检明确不冒充专有 DaVinci 集成测试；
- 真实 TC275 SIP 对比：`find_module_template(CanIf)` 输出由 125895 字符降至 8574 字符；LGK 冷启动 2.55 秒、常驻 0.17–0.18 秒，对照入口冷启动 3.28 秒、常驻 0.55–0.56 秒；单模块 `CanIf` 生成两者均约 23 秒；
- 验证：27 个 Rust 测试通过；含中文/空格目录、配置 BOM、请求 BOM、发布包和 Host 生命周期的 38 项 onboarding 连续运行 3 次全部通过；真实测试结束后 Host 正常关闭、端口释放。

## v0.3.0 - 2026-08-11

Target release tag: `v0.3.0` (not published yet)

- 将工程配置缩减为最小 `tool_path`，工程目录由 `lgk-autosar.json` 所在位置推导；多 DPA 或多 DaVinci 命令时仍可显式选择；
- 新增首次接入初始化器和非写入 doctor，并支持从任意 PowerShell 工作目录传入相对的 DPA/命令路径；
- 为 CLI/Host 增加版本一致性校验、请求结构预检、三分钟超时与自有 DaVinci 进程树清理；
- 为 resident host 增加协议版本和源码构建标识握手；发布包另带双 EXE 的 SHA-256 配对清单，明确拒绝旧 Host、部分重建的 EXE 组合或占用固定端口的其他程序；
- 修复 Windows PowerShell 首次启动时常驻 Host 继承输出句柄造成的脚本卡死；Host 忙碌或关闭期间拒绝新请求，`shutdown_host` 等待业务端口和健康端口全部释放后再返回；
- 将 DaVinci 返回的 `FAIL:` 提升为调用失败，避免生成或自动求解异常被误报成成功；
- 统一批请求的 doctor 与真实执行规则；多项数组只允许只读操作，所有写入、自动求解和生成必须单独发送；
- 支持带 UTF-8 BOM 的 Windows JSON，修复嵌套 ECUC 参数元数据串到父容器的问题，并正确索引 `ECUC-CHOICE-CONTAINER-DEF`；
- `edit_file` 强制要求精确 `expected` 原文，配置已变化时拒绝写入，并由 PowerShell 包装器保留具体错误原因；
- 新增 Windows GitHub CI、开源内容守卫、目标平台依赖许可证守卫、贡献与安全说明，以及独立 onboarding 测试；
- 发布包改为按 Git 公共候选清单逐文件复制，并用独立回归测试证明被忽略的客户 DBC/ARXML 不会进入 ZIP；
- 新增 `update_project` 和 `import_dbc`：通过 DaVinci 官方 Project Update 更新 DPA 已登记通信输入，返回耗时与日志；失败时恢复完整 Cfg 树并保留外部诊断日志，避免 DaVinci 已改 DPA/ARXML 后只恢复 DBC；
- `edit_file` 在写磁盘前正常关闭同一 resident Host 已打开的 DaVinci 会话，避免旧内存模型覆盖刚写入的 ECUC；
- 保持 MCU、SIP 和厂商定义路径无关，不在仓库中携带客户 DPA/ARXML/DBC、Vector 文件、许可证或发布二进制。

验证：格式检查和严格 Clippy 通过；26 个 Rust 测试通过；release CLI/Host 均为 v0.3.0。公开发布包从零接入测试连续运行 3 次，每次 36 项均通过。另在合法、可丢弃的 DaVinci 工程完成真实集成测试：无效 DBC 返回 1 个转换器错误、4 个警告后，完整 Cfg 逐文件验证为 0 修改、0 新增、0 缺失；有效 DBC Project Update 为 0 个转换器错误；后续 DaVinci 全工程错误列表为 0，7 个受影响模块逐个生成成功；resident host 正常关闭且两个端口释放。

限制：公开测试使用完全合成的 ECUC/SIP 夹具，不运行专有 DaVinci 生成器；doctor 是静态预检，不启动 DaVinci，也不证明许可证或生成链可用。真实 `generate_code` 和 `auto_solve_errors` 仍须在合法、匹配且可丢弃的 DaVinci/SIP 测试工程中单独验证，不能据此承诺覆盖所有 Vector 版本。现有私有 Git 历史含个人邮箱和旧名称，历史守卫会拒绝直接发布；GitHub 必须使用审计后的 clean-root 公共历史。

## v0.2.3 - 2026-08-09

Status: superseded by v0.3.0 before a standalone release.

- Accept `module_name` as a compatibility alias while keeping `module` canonical, and explain that callers must use an ECUC short name rather than a generated driver package name.
- Enforce a three-minute budget for ordinary changes: 45 seconds for DaVinci startup, 120 seconds for an operation, and 15 seconds for shutdown.
- Stop the owned DVCfgCmd process after a transport or timeout failure to prevent a stalled generation from retaining several gigabytes of memory.
- Add the CAN0/CAN1 fast path and the observed configuration, generation, batching, stale-model, and cleanup mistakes to the global skill.

Validation: `git diff --check` passed. Rust formatting, tests, and release build were not run because Cargo is not installed or discoverable on this machine.

Limitation: release binaries are not updated until the Rust toolchain is available and `cargo test --all-targets --locked` plus `cargo build --release --locked` pass.

## v0.2.2 - 2026-08-08

文档提交：`c7f2b69`

- 将原“跨工程接入与维护”说明扩展为 LGK-AUTOSAR 主使用手册；
- 补充工具用途、前置条件、中央安装、工程配置、Codex 与 PowerShell 两种调用方式；
- 为 10 个正式函数分别说明参数、返回信息、是否启动 DaVinci、示例和注意事项；
- 增加 COM 参数从查询、定位、最小修改、单模块生成、Compare 到关闭 host 的完整流程；
- 增加 ECUC/生成 C-H-LSL/业务代码/编译产物的边界，以及常见失败排查和 Git 规则。

验证：Skill 校验通过；主手册 25 段 JSON 示例全部通过实际 JSON 解析；10 个正式函数均在手册中覆盖。

限制：示例中的工程名、Signal 名、definition ref、路径和行号均为演示值，实际操作必须使用目标工程查询结果。

## v0.2.1 - 2026-08-08

实现提交：`016fc05`

- 改为 D 盘单一共享安装：所有工程共同使用 `D:\Tools\LGK-AUTOSAR`，不再把源码和程序复制进各工程；
- 新增 `Install-LGKAutosarSkill.ps1`，用 Windows 目录链接把 Codex 全局 Skill 指向同一份 D 盘源码；
- 包装器会依次使用源码根目录或 `target\release` 中的程序，兼顾本地发布和源码开发；
- 每个 AUTOSAR 工程只保存自己的 `lgk-autosar.json`，工具修改只提交到中央源码仓库；
- 更新跨工程说明、Skill 和调用示例，避免多份源码产生版本漂移。

验证：12 个 Rust 测试通过，release 构建成功；目录链接安装、Skill 校验和旧工程配置 `-ValidateOnly` 均通过。

限制：Codex 仍要求 Skill 入口位于用户 skills 目录，因此使用目录链接指向 D 盘；该链接不是第二份源码。中央目录移动后必须重新建立链接。

## v0.2.0 - 2026-08-08

实现提交：`9803f7a`

- 将工程使用中验证过的 `inspect_ecuc_containers` 合入 Rust 核心，不再由 PowerShell 临时解析 ECUC；
- 支持按模块、完整定义引用或容器名、短名正则、参数名读取参数值与引用值；
- 多条只读检查请求返回一个扁平结果数组，并拒绝与其他函数混合批处理；
- DaVinci 启动失败时读取本次临时日志，明确提示 `.dpa` 被其他程序锁定；
- 版本升级到 `0.2.0`，新增源码可见的 Skill、跨工程接入说明和发布同步脚本。

验证：`cargo test --all-targets --locked` 共 12 个测试通过；`cargo build --release --locked` 成功；在 TC275 示例工程中只读识别 8 个 `CanHardwareObject`，随后通过 `shutdown_host` 正常关闭 resident host；包装器 `-ValidateOnly` 同时验证了旧版 `LGK_*` 配置键。

限制：只读检查读取已落盘的 ECUC ARXML；DaVinci GUI 中尚未保存的修改不会出现在结果中。生成、错误列表和自动求解仍要求本机具备合法且可用的 DaVinci 环境。

## v0.1.0 - 2026-08-03

实现提交：`984adc1`

- 完成 LGK-AUTOSAR 独立命名和 Rust 源码整理；
- 支持 ECUC 模块/模板/参数定位、最小行编辑、DaVinci 校验与代码生成；
- 通过 resident host 复用 DaVinci 会话，并提供正常关闭协议；
- 模块定义路径取自当前工程，不绑定 TC275、MICROSAR 或特定厂商 SIP。

## 记录要求

后续每条记录至少包含：日期、实现提交、问题场景、改动内容、验证方法和已知限制。工程内发布包还要记录它同步自哪个源码提交。
