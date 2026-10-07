# AutoC 0.10.0 与 LGK-AUTOSAR：能力差距审查

日期：2026-10-07。审查对象是本机 `AutoC Setup 0.10.0.exe`（SHA-256：
`D9C226484642F18FB9F9FF4E3DC8814FB03671428A5A29541A26D175126F72B0`）。
原安装目录由用户 PowerShell 确认可访问，当前执行环境无法访问；初轮仅静态查看安装包清单、应用元数据及自带工作流说明，
没有运行或安装 AutoC，也没有把它的代码或资源复制进 LGK-AUTOSAR。
安装包中的 `package.json` 标识桌面应用为 `@autoc/desktop` 0.10.0；
Agent 主体以 `.jsc` 字节码分发。因此这里能确认部分接口与产品设计，
无法靠静态材料证明这些接口在某个 DaVinci 工程上的实际成功率、耗时或 token 消耗。

公开资料：[AutoC 总览](https://www.autoc-tool.com/en/guide/)、
[工作区](https://www.autoc-tool.com/en/guide/workspace)、
[ASW](https://www.autoc-tool.com/en/guide/asw)、
[DaVinci](https://www.autoc-tool.com/en/guide/davinci-configurator)、
[模块联动](https://www.autoc-tool.com/en/guide/linkage)。
另核对安装包自带的 `mapping` 工作流说明；下表只概述能力，不复用其文本或实现。

| 能力 | AutoC 0.10.0 的证据 | LGK-AUTOSAR 现状 | 判断 |
| --- | --- | --- | --- |
| BSW 写入路径 | 安装副本中可读 Groovy 体现模块级导入/替换，另有保存与同步标识 | 正式已有值修改仍走受约束的 ARXML 替换；独立 DaVinci 5 Boolean 原生实验已通过模型/保存/恢复验证 | 不能称所有 AutoC 修改都是逐参数原生写入。LGK 实验发现副本 DPA/Os 保存副作用，仅目标文件恢复；未验收模块导入或通用写入，不切换默认路径。 |
| ASW 建模 | 官网列出数据类型、接口、Port、SWC 和 Internal Behavior 的创建/编辑 | 已新增 typed ASW 专用文件创建/更新/删除：四种 Implementation Type、SR/CS Interface、Application SWC、Port、Runnable/Timing Event/数据访问、Composition；保留已有引用修改 | 非客户许可样本通过 DaVinci 5 重载、15 个关键对象/类型及导出周期复核，删除后包消失；尚缺 Application Type 等完整模型、持久撤销、RTE 生成验收。不能称全部补齐。 |
| Port/Data/Task 映射 | 官网有三个独立视图；安装包工作流区分连接、信号映射和 RTE–OS 任务映射 | 已有保存条目的只读汇总/分页；新 ASW Bundle 可声明本文件实例间 Assembly/Delegation 连接，检查方向和接口 | 实际工程只读核对了 68/18/136 条保存映射；新连接通过隔离样本原生加载。仍缺跨已有 SWC 的完整 map/unmap、Data–Signal 及 RTE–OS 写入，不把缺 task-ref 当错误。 |
| 大工程输出 | 官网按模块加载 ECUC，并有 ASW 文件排除设置 | 有按模块 ECUC 查询、汇总查询及部分接口分页；本轮为 inspect_ecuc_containers 补分页与字节预算 | 先前 2,000 条合成查询约 670 KB 且 limit 无效。新增分页/超大字段合成验收已通过，不能笼统说所有接口已有上限，也不代表实测 Agent token 节省。 |
| 跨工具链 | 官网支持 EB、DaVinci、ETAS 等模块归属和同步 | 仅服务 Vector DaVinci | 用户当前工程以 Vector 为主，暂不值得引入多工具同步风险。 |
| 改动历史与撤销 | 官网有按路径的改动列表和单项撤销 | Git、变更记录、精确旧值及部分失败回滚；已设计持久撤销协议，尚无跨请求撤销 API | 须实现并验收旧/新值、工程身份、写后文件哈希、外部修改拒绝和中断恢复；不能简单做“反向覆盖”，原生单值恢复也不等于整工程撤销。 |
| Agent | 桌面包有 Agent 字节码、会话与技能；官网展示自然语言操作 | 由 Codex 承担推理，LGK 是本地受约束桥梁 | 无同工程、同任务、同模型额度的基准测试，不能断言哪方更快或更省 token。 |

本轮先补 `audit_autosar_model`：只读扫描保存后的 System/Developer 模型，
统计当前扫描范围内没有目标的 `*-REF` / `*-TREF`，按角色汇总，默认仅返回
8 个例子，最多 32 个。它复用现有有界索引，不增第三方 API 请求，也不改
ECUC/ASW。缺失目标可能位于范围外或 ECUC/SIP，因此字段明确命名为
`unresolved_in_scope`，绝不自动修复或宣称 DaVinci 校验失败。

后续优先级：已有 Port/Data/Task 保存条目的只读摘要、常用 ASW 文件写入闭环、
单 Boolean 原生实验和撤销协议设计。接下来核对原生保存集并实现持久记录与安全撤销；
ASW 的剩余类型、Data–Signal/RTE–OS 映射和 RTE 生成仍需独立实现及真实验收。
常用 ASW 的范围/证据见 `docs/ASW写入与验收.md`，不从安装包推断实现。

补充样本：用户复制的安装访问副本位于仓库外的 ReverseEngineering 目录。
其 app.asar SHA-256 为 `3CE09471A39002E69BFE624A1D9C174F8F9BB23826FE80A550E69F17C15D3F4D`，
与早先安装包提取的 ASAR 逐字节一致；没有安装版原 EXE 哈希，不能推断 EXE 相同。
后续可读界面 JS 与 IPC/字节码标识交叉核对揭示引用候选、结构化列表操作、
操作树及按 ID 撤销等方向，仍无 AutoC 运行验收。原始专有资源全部保留仓库外。

阶段实验与撤销的证据/限制见 `docs/原生写入实验与安全撤销设计.md`。
原生实验使用非客户示例的隔离副本，未与 AutoC 在同一工程做运行对比。
