# LGK-AUTOSAR 运行说明

操作 Vector DaVinci ECUC 工程前，先读取同目录的 `SKILL.md`。使用同目录的
PowerShell 包装器，不要只单独调用某一个 EXE，也不要直接结束 Host 进程。

目标工程的 Cfg 目录需要 `lgk-autosar.json`；若没有，使用
`Initialize-LGKAutosarProject.ps1` 创建。修改前先查询，保持改动范围最小，
只生成受影响模块，并在每次会话结束时调用 `shutdown_host`。

只读映射概览用 `inspect_autosar_mapping`；先汇总，再按类别及路径细查。
缺 task-ref 和范围内未解析引用都需要核查依据，不能称为 DaVinci 校验错误。

ASW 创建/整包更新/删除用 `write_asw_bundle`，先读 SKILL 中的声明格式；目标必须是
工程内 DPA 已登记应用输入中的专用 LGK 文件。先保存关闭 GUI 并 preview，创建
使用 expected:null，更新/删除使用完整旧内容。写后重查并按需 DaVinci 验证；
单个真实样本的模型加载/导出通过不代表 RTE 生成和 OS 调度已验收。

本包兼容 Codex、OpenCode、Claude Code 以及任何能读取本文件并运行
PowerShell 的 Agent。包内不包含 Vector 软件、SIP、许可证或客户配置。
