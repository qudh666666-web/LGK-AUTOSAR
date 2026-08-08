# LGK-Vector 更新记录

版本号说明发布顺序，Git 提交号用于定位准确源码。每次功能、接口、包装器或 Skill 改动，都必须在顶部新增记录。

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
