# LGK-Vector 更新记录

版本号说明发布顺序，Git 提交号用于定位准确源码。每次功能、接口、包装器或 Skill 改动，都必须在顶部新增记录。

## v0.3.0 - 2026-08-11

Target release tag: `v0.3.0` (not published yet)

- 将工程配置缩减为最小 `tool_path`，工程目录由 `lgk-vector.json` 所在位置推导；多 DPA 或多 DaVinci 命令时仍可显式选择；
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
- 保持 MCU、SIP 和厂商定义路径无关，不在仓库中携带客户 DPA/ARXML/DBC、Vector 文件、许可证或发布二进制。

验证：格式检查和严格 Clippy 通过；23 个 Rust 测试通过；release CLI/Host 均为 v0.3.0；发布包从零接入测试连续运行 3 次，每次 34 项均通过，doctor 为 0.040–0.141 秒、首次本地请求为 1.420–1.453 秒、完整一轮为 6.322–6.535 秒；发布清单 5 项隔离断言、开源内容与 Windows GNU 目标 34 个依赖包的许可证守卫通过；resident host 每轮均正常关闭并释放两个端口。另在合法的本机 DaVinci 环境完成了一次只读 `get_errors_list`，未改 ECUC，也未生成 C/H/LSL。

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

- 将原“跨工程接入与维护”说明扩展为 LGK-Vector 主使用手册；
- 补充工具用途、前置条件、中央安装、工程配置、Codex 与 PowerShell 两种调用方式；
- 为 10 个正式函数分别说明参数、返回信息、是否启动 DaVinci、示例和注意事项；
- 增加 COM 参数从查询、定位、最小修改、单模块生成、Compare 到关闭 host 的完整流程；
- 增加 ECUC/生成 C-H-LSL/业务代码/编译产物的边界，以及常见失败排查和 Git 规则。

验证：Skill 校验通过；主手册 25 段 JSON 示例全部通过实际 JSON 解析；10 个正式函数均在手册中覆盖。

限制：示例中的工程名、Signal 名、definition ref、路径和行号均为演示值，实际操作必须使用目标工程查询结果。

## v0.2.1 - 2026-08-08

实现提交：`016fc05`

- 改为 D 盘单一共享安装：所有工程共同使用 `D:\Tools\LGK-Vector`，不再把源码和程序复制进各工程；
- 新增 `Install-LGKVectorSkill.ps1`，用 Windows 目录链接把 Codex 全局 Skill 指向同一份 D 盘源码；
- 包装器会依次使用源码根目录或 `target\release` 中的程序，兼顾本地发布和源码开发；
- 每个 AUTOSAR 工程只保存自己的 `lgk-vector.json`，工具修改只提交到中央源码仓库；
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

- 完成 LGK-Vector 独立命名和 Rust 源码整理；
- 支持 ECUC 模块/模板/参数定位、最小行编辑、DaVinci 校验与代码生成；
- 通过 resident host 复用 DaVinci 会话，并提供正常关闭协议；
- 模块定义路径取自当前工程，不绑定 TC275、MICROSAR 或特定厂商 SIP。

## 记录要求

后续每条记录至少包含：日期、实现提交、问题场景、改动内容、验证方法和已知限制。工程内发布包还要记录它同步自哪个源码提交。
