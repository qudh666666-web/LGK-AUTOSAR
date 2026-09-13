---
name: lgk-vector
description: 用于快速、可验证地完成 Vector DaVinci ECUC 查询、修改、生成、DBC 导入和常驻 Host 正常关闭。
---

# LGK-Vector

所有请求都使用同目录的 `Invoke-LGKVector.ps1` 包装器。本运行包要求电脑中
已合法安装匹配的 DaVinci/SIP，并且目标工程 Cfg 目录内存在
`lgk-vector.json`；若配置不存在，先使用同目录的
`Initialize-LGKVectorProject.ps1`。

## 默认快速流程

1. 只查询必要内容：`find_module`、`get_param_definition`、
   `locate_container` 或 `inspect_ecuc_containers`。
2. 修改 ECUC 前，保存并关闭同一个 DaVinci GUI 工程，读取准确原文后发送
   一条带 `expected` 的小范围 `edit_file` 请求。
3. 只对受影响模块调用 `generate_code`。若失败，再读取该模块的
   `get_errors_list`；不要盲目重试或全量生成。
   必须传 `module` 或兼容字段 `module_name`；全量明确传 `module:"all"`。
   工具自动验收本次 Generation Report，成功须有 `passed:true`，无需再次读取
   已通过的报告。模块按工程真实定义路径匹配；报告缺失、过期、歧义或格式不支持
   会报错，不自动重试。失败时先看报告路径及原因。
4. 同步或交付前调用 `verify_delivery`，核对实际编译文件与生成文件完全一致，
   并验证本任务要求出现或禁止出现的关键文本；必须得到 `passed:true`。
5. 分开汇报 ECUC 配置改动与生成的 C/H/LSL 输出。
6. 每次会话结束都通过包装器发送 `{ "func": "shutdown_host" }`。

## 必须遵守

- v0.3.9 的交付校验失败已包含失败项和文本明细，不为相同信息再跑一次。
  列表最多 8 项，每类文本最多 8 条，超出时按总数缩小查询范围。
- 模块找不到时先检查错误中的当前工程候选；候选仅为提示，不自动改名生成。
  错误仍以异常/非零退出返回，结构化 JSON 位于错误详情，成功格式保持原样。

- 能使用 LGK-Vector 完成的 DaVinci ECUC 改动，不要手工改写 ARXML。
- `edit_file`、`import_dbc`、`update_project`、`auto_solve_errors`、
  `generate_code` 和 `shutdown_host` 必须各自单独发送，不能混在数组请求中。
- `auto_solve_errors` 必须先有最新错误列表、用户明确同意，并传入
  `confirmed:true`。
- `import_dbc` 或 `update_project` 前，保存并关闭 DaVinci GUI。
- 仅使用支持的函数：`inspect_ecuc_containers`、`diff_ecuc`、`find_module`、
  `find_module_template`、`get_param_definition`、`locate_container`、
  `verify_delivery`、`edit_file`、`get_errors_list`、`auto_solve_errors`、`generate_code`、
  `update_project`、`import_dbc`、`shutdown_host`。
- 比较两个工程内 ECUC 快照时用 `diff_ecuc`。必须指定一个实际模块和两份
  `.arxml`，优先加 `path_prefix`；默认最多 32 项，硬上限 256 项，单个值最多
  返回 512 个 Unicode 字符。重复语义路径会拒绝比较，不会猜测配对。

## 示例

```powershell
& "<skill-root>\Invoke-LGKVector.ps1" `
  -ProjectPath "D:\\Work\\Vehicle\\Cfg" `
  -Request '{"func":"inspect_ecuc_containers","module":"Com","container":"ComSignal"}'
```

交付前检查示例：

```powershell
& "<skill-root>\Invoke-LGKVector.ps1" `
  -ProjectPath "D:\\Work\\Vehicle\\Cfg" `
  -Request '{"func":"verify_delivery","root":"D:\\Work\\Vehicle","checks":[{"path":"Proj_Code\\Gen\\Can_Lcfg.c","same_as":"Proj_Config\\Gen\\Can_Lcfg.c","must_contain":["CanIsr_0"],"must_not_contain":["CanIsr_1"]}]}'
```

跨 Agent 接入方式和工程工作规则见同目录的 `AGENTS.md`。
