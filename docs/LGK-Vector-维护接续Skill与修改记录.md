# LGK-Vector：下一对话维护 Skill 与修改记录

## 2026-09-13 后续维护补记

本机现为 v0.3.8，实现提交 `4e1e39a`。三项变更：自动验收本次生成报告、
同目录临时文件重命名替换、生成请求必须显式提供模块。现有 `module_name`
兼容和真实定义发现保留，全量使用 `module:"all"`。成功仅返回报告摘要，
不要求 Agent 再读全文；失败不自动重跑。详细限制见 CHANGELOG v0.3.8。

本轮按 Build-LGKVector.ps1 重建两份 EXE（build=dev），随后复用脚本设置的
当前进程环境运行 cargo test。默认工具链缺 rustfmt/clippy，使用 `.toolchain/
fresh-rustup/toolchains/stable-x86_64-pc-windows-gnu/bin` 中已安装组件完成检查，
没有安装组件或更改系统环境。40 项 Rust、51 项接入断言、13 项包清单断言及
当前内容守卫通过；另两份历史真实报告只读检查通过，未进行真实 DaVinci 生成。
客户工程及公开仓库未修改。下文版本与发布情况属于此前记录。

> 用途：把本文件作为下一段对话的首个上下文文件。它记录本轮已完成的维护、当前仓库状态、构建 EXE 的唯一方法、测试证据，以及容易踩到的边界。它是中央维护仓库的内部交接资料，不是 GitHub Release 面向普通使用者的说明。

## 0. 下一对话的启动指令

先让下一位智能体完整读取本文件，再读取以下三个权威文件：

- `D:\Tools\LGK-Vector\SKILL.md`
- `D:\Tools\LGK-Vector\AGENTS.md`
- `D:\Tools\LGK-Vector\docs\对话衔接与项目现状.md`（第 10 节尤其重要）

可直接发送：

```text
请先完整读取并遵守：
D:\Tools\LGK-Vector\docs\LGK-Vector-维护接续Skill与修改记录.md

然后完整读取：
- D:\Tools\LGK-Vector\SKILL.md
- D:\Tools\LGK-Vector\AGENTS.md
- D:\Tools\LGK-Vector\docs\对话衔接与项目现状.md

继续维护/使用 LGK-Vector。先检查中央与公开仓库 Git 状态；保留已有改动，禁止 reset、clean 或覆盖无关文件。ECUC 修改必须通过 Invoke-LGKVector.ps1，先查询后最小修改；若本次启动过 Host，结束时单独发送 {"func":"shutdown_host"}。
```

## 1. 仓库、版本与当前提交状态

| 项目 | 路径 / 地址 | 当前状态（本记录写入时） |
| --- | --- | --- |
| 中央维护源码 | `D:\Tools\LGK-Vector` | 分支 `codex/open-source-readiness`；HEAD `cfead43`；工作区干净；无配置远端。 |
| 公开源码 | `D:\Tools\LGK-Vector-Public` | 分支 `main`；已推送 HEAD `78d44c0`；工作区干净。 |
| GitHub | `https://github.com/qudh666666-web/LGK-Vector` | `main` 已含 v0.3.6/v0.3.7 源码同步；最新正式 GitHub Release 仍是 v0.3.5。 |

不要把“公开源码已到 v0.3.7”误解为“已发布 v0.3.7 Release ZIP”。本轮没有打包、创建 tag 或发布 Release；普通使用者仍应下载现有 Release ZIP，而不是从中央仓库复制 `target`。

## 2. 本轮和前序对话已经完成的改动

### 2.1 PowerShell 包装器约束补齐（v0.3.5 后维护）

此前已在中央与公开仓库提交以下改动：

- 中央提交：`7bc01adaed3dbc5e9f4b73110999d80a55b5dd76`
- 公开仓库对应提交：`b0d35e83256ab1d16bd50560a03da52179621f7e`
- 提交主题：`Reject mutating request batches in wrapper`

涉及 `scripts\Invoke-LGKVector.ps1`、安装/打包脚本、onboarding 与 package smoke 测试、CHANGELOG 和主交接文档。具体行为：

1. 多项 JSON 数组只能包含只读请求。`edit_file`、`auto_solve_errors`、`generate_code`、`update_project`、`import_dbc`、`shutdown_host` 出现在多项数组时，包装器会在启动 Host 前报错。
2. `Set-StrictMode -Version Latest` 与 `$ErrorActionPreference = 'Stop'` 已移至 `param(...)` 后的最前面。
3. 包装器失败信息会带退出码与请求文件路径。
4. 白名单注释明确 Rust daemon 是权威实现，避免 PowerShell 与 Rust 函数表长期漂移。
5. 安装脚本说明了 Codex、Claude Code、OpenCode 的常见 Skill 目录。
6. 发布包守卫拒绝 `.pdb` 调试符号文件。

### 2.2 中央已有、现已同步到公开仓库的 v0.3.6 / v0.3.7

中央源码中的以下功能本来已存在，本轮将其完整公开源码同步到 GitHub：

- v0.3.6：提交 `438fef9`，新增只读 `verify_delivery`。它验证生成文件和编译目录文件是否逐字节一致，支持必含/禁含文本断言；只接受受控项目根下的相对路径，拒绝 `..`、盘符、链接逃逸、超大文件，并且不返回文件正文。
- v0.3.7：提交 `530a744`，修复常驻 Host 在 PowerShell、Git Bash/MSYS、.NET 重定向管道中可能继承外层句柄、导致调用方永远等不到 EOF 的问题。Windows 下改用 `CreateProcessW`、`bInheritHandles=FALSE`、空标准流、`DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`；同时加入 Windows 命令行引号测试。
- v0.3.7：`Invoke-LGKVector.ps1` 给 CLI 调用增加超时看门狗（版本检查、Host 启动、doctor、普通请求分别有上限）；超时会终止子进程树并保留部分输出。
- v0.3.7：`Build-LGKVector.ps1`、打包脚本及 smoke 测试针对中文 Windows PowerShell 5.1 进行 UTF-8 BOM/控制台编码加固。

公开同步提交：

```text
78d44c0 Sync v0.3.6 and v0.3.7 source updates
```

该提交改动 21 个公开文件，包含 Rust、PowerShell、测试、CHANGELOG、Skill、README 和 CAN 说明；没有 EXE、DaVinci、SIP、许可证、客户工程或生成代码。

## 3. 本轮构建 EXE 的方法与证据

### 3.1 唯一允许的重建命令

只有修改 Rust 源码后才需要重建 EXE。必须在中央源码仓库运行：

```powershell
Set-Location "D:\Tools\LGK-Vector"
& ".\scripts\Build-LGKVector.ps1"
```

不要手动调用 Cargo 进行 EXE 构建；不要手动设置 `CARGO_HOME`、`RUSTUP_HOME`、`DLLTOOL` 或 `PATH`；不要安装或排查 GNU、GCC、dlltool、MSYS2、`link.exe`、Visual Studio Build Tools。构建脚本在当前 PowerShell 进程内处理项目私有工具链，默认 `--offline`，不会改系统环境变量。

只有脚本明确提示 Rust 依赖缓存缺失，并且用户明确授权联网时，才可使用：

```powershell
& ".\scripts\Build-LGKVector.ps1" -AllowNetwork
```

成功后应存在并可执行：

```text
D:\Tools\LGK-Vector\target\release\lgk-vector.exe
D:\Tools\LGK-Vector\target\release\lgk-vector-host.exe
```

本轮成功输出：

```text
lgk-vector 0.3.7 protocol=2 build=dev
lgk-vector-host 0.3.7 protocol=2 build=dev
```

### 3.2 遇到的构建坑：`liblgk_vector.d` 映射占用

最初三次运行构建脚本都在 Cargo 完成 release 编译阶段后失败：

```text
failed to create file D:\Tools\LGK-Vector\target\release\liblgk_vector.d
os error 1224：请求的操作无法在使用用户映射区域打开的文件上执行
```

已确认的事实：

- 构建脚本只调用一次顺序化 `cargo build --release --locked --offline`，没有脚本层并发构建。
- 当时不存在残留 `cargo`、`rustc`、`lgk-vector` 或 `lgk-vector-host` 进程。
- Windows Restart Manager 没有报告持续锁定者，文件也可通过独占读写探测；因此更像构建期间的短暂外部映射占用，而不是依赖缺失或工具链配置问题。

实际恢复动作是由用户手动删除这一份可再生依赖元数据：

```powershell
Remove-Item -LiteralPath "D:\Tools\LGK-Vector\target\release\liblgk_vector.d" -Force
```

随后使用原构建脚本成功。下一位智能体不要对整个 `target` 执行 `clean`，更不能 `git reset --hard`。若再次遇到完全相同的 `os error 1224`，先保存错误、确认只命中这个精确的 `.d` 文件、不要尝试系统级工具链修复；由于当前自动化环境可能拒绝删除命令，应请用户手动执行上述精确删除后再运行原脚本。

## 4. 本轮验证证据

本轮实际执行并通过：

| 位置 | 检查 | 结果 |
| --- | --- | --- |
| 中央 | `Build-LGKVector.ps1` | 成功；两个 EXE 均输出 `0.3.7 protocol=2 build=dev`。 |
| 中央 | `tests\open-source\Invoke-PackageManifestSmoke.ps1` | 13 项断言通过。 |
| 中央 | `tests\onboarding\Invoke-OnboardingSmoke.ps1` | 50 项测试通过，含 Host 生命周期/管道回归路径。 |
| 公开 | `tests\open-source\Invoke-OpenSourceGuard.ps1 -IncludeHistory` | 通过；66 个跟踪文件、0 个禁入文件、0 项敏感内容匹配。 |
| 公开 | `tests\open-source\Invoke-PackageManifestSmoke.ps1` | 13 项断言通过。 |

本轮没有单独运行 Rust `cargo test`、`fmt` 或 `clippy`，原因是当前维护约束要求 EXE 构建只能走构建脚本，且历史记录中已有 v0.3.7 Rust 回归验证。不要把历史测试结果冒充为本轮重新执行；若后续有 Rust 改动，应按当时有效的维护规则补做必要 Rust 验证。

## 5. 运行架构与调用链（维护者重点）

```text
Agent / PowerShell
  └─ scripts\Invoke-LGKVector.ps1
       ├─ JSON 形状、函数白名单、批量只读约束
       ├─ static doctor / 请求文件 / 输出与超时治理
       └─ target\release\lgk-vector.exe
            ├─ 仅本地读取：inspect_ecuc_containers 直接解析 ECUC ARXML
            ├─ 常驻请求：--start-host 启动 lgk-vector-host.exe
            └─ HTTP/令牌请求转发
                 └─ daemon::commands（请求调度、权限与函数注册）
                      ├─ ops（高层业务操作：模板、编辑、生成、导入、验证）
                      ├─ project（项目/DPA/ECUC/路径解析与边界校验）
                      └─ vector（DaVinci CLI 交互、报告与受控外部调用）
                           └─ 用户本机已授权的 Vector DaVinci / SIP
```

职责分界：

- `Invoke-LGKVector.ps1`：给 Agent 使用的安全入口。它做 PowerShell 侧参数检查、请求文件管理、Host 启动、子进程超时和错误上下文；不是业务逻辑的唯一真相。
- Rust CLI（`src/app/cli.rs`）：短生命周期前台程序，doctor、静态本地读操作、Host 启动与 HTTP 转发都在此层；v0.3.7 的 Windows Host 隔离修复在这里。
- 常驻 Host（`lgk-vector-host.exe`）：保存会话和受控 DaVinci 调用状态；最终必须用独立 `shutdown_host` 正常关闭。
- `daemon`（特别是 `src/daemon/commands.rs`）：函数注册和请求调度的权威层。PowerShell 白名单只能尽早报错，不能替代 Rust 侧校验。
- `ops`：面向功能的操作实现。`verify_delivery` 位于 `src/ops/verify_delivery.rs`；修改/生成/导入等也在这里编排。
- `vector`：DaVinci CLI 调用、日志/报告解析等 Vector 专用边界。不要把 DaVinci 专有行为散落进 PowerShell 或直接文本编辑。
- `project`：项目文件发现、路径规范化与目录边界。所有可能触碰工程文件的操作都应经过这一层的约束。

## 6. ECUC 操作 Skill：强制流程

1. 先检查两个仓库 Git 状态，保留已有变更；禁止 `reset`、`clean`、覆盖无关文件。
2. 涉及 ECUC 时，必须用 `Invoke-LGKVector.ps1`；禁止手改 ARXML。
3. 先执行只读查询（模块、容器、参数定义、错误列表或 `inspect_ecuc_containers`），确认精确对象和现值。
4. 只做用户明确授权的最小改动；变更函数必须是独立 JSON 请求，不能和其他请求组成数组。
5. `edit_file`、`update_project`、`import_dbc` 前，保存并关闭同一个 DaVinci GUI 工程；仅在需要时执行生成或 Project Update。
6. 修改后再次查询，再按受影响模块执行生成/验证；不要把“包装器返回完成”直接当作“DaVinci 无错误”。需要检查最新 Generation Report 的验证错误和 `GENERATION` 阶段。
7. 每次实际项目变更都在项目内维护 `docs\LGK-Vector-变更记录.md`：目标、修改前后查询、请求摘要、生成/更新结果、错误、风险、下一步。不要记录凭据、许可证、客户路径或完整敏感 ARXML。
8. 如果本次启动过 Host，结束时必须单独发送：

```json
{"func":"shutdown_host"}
```

## 7. 多项请求与变更请求的正确写法

允许的多项数组示例（只读）：

```json
[
  {"func":"find_module","module":"Can"},
  {"func":"get_errors_list"}
]
```

禁止的混合示例：

```json
[
  {"func":"find_module","module":"Can"},
  {"func":"generate_code","module":"Can"}
]
```

应改为先发只读请求，再单独发送：

```json
{"func":"generate_code","module":"Can"}
```

以下函数永远独立发送：`edit_file`、`auto_solve_errors`、`generate_code`、`update_project`、`import_dbc`、`shutdown_host`。

## 8. Git、公开化与发布边界

- 中央仓库是维护工作区，公开仓库是 GitHub 发布源码快照；两者不要混为一谈。
- 修改工具实现时：先做最小改动，再更新相关测试、`CHANGELOG.md` 与 `docs\对话衔接与项目现状.md`；本文件只记录本轮接续事实，不能替代上述正式交接文档。
- 只暂存本次任务涉及文件；提交前至少检查 `git diff --check`、暂存清单和测试结果。
- 没有用户明确授权时，不推送、不开 PR、不创建 tag、不发布 Release。
- 打 Release 包时才运行 `scripts\Sync-LGKVectorPackage.ps1 -IncludeBinaries`，随后执行包内 EXE 自检。不要把中央 `target` 目录或 `.pdb`、日志、DPA、ARXML、DBC、SIP/DaVinci 内容提交到公开仓库。
- 普通使用者路径：下载 GitHub Release ZIP，运行 `test\一键测试EXE.cmd`，复制完整 `lgk-vector` 文件夹。普通使用者不需要 Rust、Cargo、GCC、MSYS2 或源码构建。

## 9. 当前已知限制与下一步建议

- GitHub `main` 已有源码 v0.3.7，但 Release ZIP 仍是 v0.3.5；若用户要求发布 v0.3.7，必须另行执行完整打包、自检、Git tag 和 Release 流程，不能把本轮源码 push 当作发布完成。
- 中央维护仓库的公开内容守卫若带 `-IncludeHistory` 可能按设计失败，因为私有历史不等同于公开净化历史；公开仓库可使用 `-IncludeHistory` 守卫。
- 当前构建输出的 build ID 是 `dev`。若要可追溯的对外二进制，需要按正式发布流程生成并记录对应版本/提交，不要直接把 `build=dev` 当作最终发行物。
- v0.3.6/0.3.7 中包含 TC275 CAN0 运行手册和相关说明；它们是已验证案例，不是所有 DaVinci/SIP/板卡的通用针脚规则。只有用户明确处于相同硬件/项目上下文时才读取并使用对应参考。

## 10. 给下一位智能体的最短行动清单

1. 读本文件、`SKILL.md`、`AGENTS.md`、主交接文档第 10 节。
2. 运行两个仓库的 `git status --short --branch`；不要假设干净或已推送。
3. 明确本次任务是“ECUC 工程操作”还是“LGK-Vector 工具维护”，不要混合扩大范围。
4. ECUC：先查再改、独立变更请求、记录前后证据、最后关 Host。
5. 工具源码：最小改动、先执行 `Build-LGKVector.ps1`、再跑相关测试、更新正式文档、只提交任务文件。
6. 公开化/发布：先检查 IP 与发布范围，只有明确授权才 push/tag/release。
