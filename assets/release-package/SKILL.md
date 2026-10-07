---
name: lgk-autosar
description: 用于快速、可验证地完成 Vector DaVinci ECUC 查询、修改、生成、DBC 导入和常驻 Host 正常关闭。
---

# LGK-AUTOSAR

所有请求都使用同目录的 `Invoke-LGKAutosar.ps1` 包装器。本运行包要求电脑中
已合法安装匹配的 DaVinci/SIP，并且目标工程 Cfg 目录内存在
`lgk-autosar.json`；若配置不存在，先使用同目录的
`Initialize-LGKAutosarProject.ps1`。

## 默认快速流程

1. 只查询必要内容：`find_module`、`get_param_definition`、
   `locate_container` 或 `inspect_ecuc_containers`。
2. 修改 ECUC 前，保存并关闭同一个 DaVinci GUI 工程。已有参数或引用优先用
   `set_ecuc_value` 并传准确 `expected`；多项已有值可用单独的 `set_ecuc_values`
   请求，先以 `preview:true` 预检；新增/删除结构才用小范围 `edit_file`。
3. 只对受影响模块调用 `generate_code`。若失败，再读取该模块的
   `get_errors_list`；不要盲目重试或全量生成。
   必须传 `module` 或兼容字段 `module_name`；全量明确传 `module:"all"`。
   工具自动验收本次 Generation Report，成功须有 `passed:true`，无需再次读取
   已通过的报告。模块按工程真实定义路径匹配；报告缺失、过期、歧义或格式不支持
   会报错，不自动重试。失败时先看报告路径及原因；校验错误会带出报告中的
   错误码和目标路径（如果报告提供）。成功响应的 `generated_files` 列出本次
   内容变化；可传 `delivery:{"root":"<工程绝对目录>","directory":"<相对交付目录>"}`
   标记变化文件是否仍需同步，不会自动复制。
4. 同步或交付前调用 `verify_delivery`，核对实际编译文件与生成文件完全一致，
   并验证本任务要求出现或禁止出现的关键文本；必须得到 `passed:true`。
5. 分开汇报 ECUC 配置改动与生成的 C/H/LSL 输出。
6. 每次会话结束都通过包装器发送 `{ "func": "shutdown_host" }`。

## 必须遵守

- v0.3.9 的交付校验失败已包含失败项和文本明细，不为相同信息再跑一次。
  列表最多 8 项，每类文本最多 8 条，超出时按总数缩小查询范围。
- 模块找不到时先检查错误中的当前工程候选；候选仅为提示，不自动改名生成。
  错误仍以异常/非零退出返回，结构化 JSON 位于错误详情，成功格式保持原样。

- 能使用 LGK-AUTOSAR 完成的 DaVinci ECUC 改动，不要手工改写 ARXML。
- `write_asw_bundle`、`set_asw_reference`、`set_ecuc_value`、`set_ecuc_values`、`edit_file`、`import_dbc`、`update_project`、`auto_solve_errors`、
  `generate_code` 和 `shutdown_host` 必须各自单独发送，不能混在数组请求中。
- `auto_solve_errors` 必须先有最新错误列表、用户明确同意，并传入
  `confirmed:true`。
- `import_dbc` 或 `update_project` 前，保存并关闭 DaVinci GUI。
- 仅使用支持的函数：`inspect_ecuc_containers`、`inspect_autosar_model`、`audit_autosar_model`、`inspect_autosar_mapping`、`trace_autosar_model`、`set_asw_reference`、`write_asw_bundle`、`diff_ecuc`、`set_ecuc_value`、`set_ecuc_values`、`find_module`、
  `find_module_template`、`get_param_definition`、`locate_container`、
  `verify_delivery`、`edit_file`、`get_errors_list`、`auto_solve_errors`、`generate_code`、
  `update_project`、`import_dbc`、`shutdown_host`。
- 比较两个工程内 ECUC 快照时用 `diff_ecuc`。必须指定一个实际模块和两份
  `.arxml`，优先加 `path_prefix`；默认最多 32 项，硬上限 256 项，单个值最多
  返回 512 个 Unicode 字符。重复语义路径会拒绝比较，不会猜测配对。
- 大型 DBC 导入后，先用
  `{"func":"inspect_autosar_model","scope":"model","summary_only":true}`
  获取 System/Developer 对象类型计数（最多显示 64 类），不要把所有信号逐条
  送进对话。此结果不是新旧 DBC 差异，也不能证明映射正确；异常对象再定向查询。
- `audit_autosar_model` 只读统计当前扫描范围内缺失目标的 `*-REF` / `*-TREF`，
  默认仅回传 8 个例子，最多 32 个。可用 `kinds` 和 `path_prefix` 缩小源对象；
  范围外、ECUC 或 SIP 中的目标也可能被列为缺失，不能直接当作 DaVinci 错误。
- ASW 已有引用先用 `inspect_autosar_model` / `trace_autosar_model` 定位。
  `set_asw_reference` 仅修改 `Config/Developer` 或 DPA 已登记应用输入中唯一对象的一个现有
  `*-REF` / `*-TREF`；传入文件、语义路径、对象种类、引用角色、精确旧路径和
  新路径。先 `preview:true`，写入后再次查询。新目标须已存在且种类匹配
  `DEST`；该接口不创建 SWC、Port、Interface、Data Type 或 Mapping。
- `write_asw_bundle` 用于独立 LGK ASW 文件的创建、整份声明更新或删除；目标目录
  须在工程内且已登记于 DPA `Folders/ApplicationComponentFolders`。写前保存关闭
  同一 GUI 工程并先 preview；创建须 expected:null，更新/删除须完整旧 UTF-8 内容。
  bundle 支持实现类型 scalar/alias/array/record、SR/CS 接口、SWC/P/R Port、
  Runnable/SR 读写/Timing Event，以及包内 Composition 的 assembly/delegation。
  示例见下文。外部正在引用的对象不允许整包改动，不强制覆盖。
  写入结果 davinci_validated:false；真实 DaVinci 5 样本已验证创建、改周期和删除后
  重载/导出，但未验收 RTE 生成、应用 C 代码或 RTE-OS 任务映射。
  model/developer 查询还覆盖工程内已登记的应用输入；外部/不存在目录不在扫描范围内。
- `set_ecuc_value` 按模块、容器实例路径和参数/引用名修改一个已有值，必须提供
  保存到磁盘的准确 `expected`。目标缺失、重复或旧值变化时拒绝；写入前后解析
  复核并原子替换，不重排无关 XML。新值最多 4096 个 Unicode 字符。
- `set_ecuc_values` 的 `edits` 最多 32 项，涉及最多 8 个文件/128 MiB；
  写入前整组预检，后续文件失败时尽力回滚先前文件，不保证断电原子性。
  修改后必须再次查询。`generated_files` 只是同步提示；最终仍用
  `verify_delivery` 逐字节验证实际编译文件。

## 示例

创建预览示例（目标目录须已存在并登记，type_ref 须在当前扫描范围存在）：

```json
{"func":"write_asw_bundle","file":"D:/Work/Vehicle/Cfg/Config/ApplicationComponents/NewAsw.arxml","expected":null,"preview":true,"bundle":{"package":"NewAsw","types":[{"kind":"alias","name":"Count","type_ref":"/AUTOSAR_Platform/ImplementationDataTypes/uint8"}],"interfaces":[{"kind":"sender_receiver","name":"Values","data_elements":[{"name":"Value","type_ref":"/NewAsw/Count"}]}],"components":[{"name":"Producer","ports":[{"name":"Out","direction":"provide","interface_ref":"/NewAsw/Values"}],"runnables":[{"name":"Tick","symbol":"Producer_Tick","period_seconds":0.01,"writes":[{"port":"Out","data_element":"Value"}]}]}]}}
```

更新使用同样 bundle 并给出完整旧文件；删除以 delete:true 代替 bundle。
旧内容在本机用 `[Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes($file))`
读取并构造 JSON，无须将整份文件送进 Agent。仅支持 LGK 标记的专用文件，不能
把厂商或客户原有 ASW 文件转成整包覆盖目标；结构创建不代表 ECU 已分配或任务已调度。

```powershell
& "<skill-root>\Invoke-LGKAutosar.ps1" `
  -ProjectPath "D:\\Work\\Vehicle\\Cfg" `
  -Request '{"func":"inspect_ecuc_containers","module":"Com","container":"ComSignal"}'
```

交付前检查示例：

```powershell
& "<skill-root>\Invoke-LGKAutosar.ps1" `
  -ProjectPath "D:\\Work\\Vehicle\\Cfg" `
  -Request '{"func":"verify_delivery","root":"D:\\Work\\Vehicle","checks":[{"path":"Proj_Code\\Gen\\Can_Lcfg.c","same_as":"Proj_Config\\Gen\\Can_Lcfg.c","must_contain":["CanIsr_0"],"must_not_contain":["CanIsr_1"]}]}'
```

跨 Agent 接入方式和工程工作规则见同目录的 `AGENTS.md`。

### Compact container queries (v0.4.1)

`inspect_autosar_mapping` 只读查询保存的 Port/Data/SWC 与 BSW 任务映射条目。
先用 `category:all, summary_only:true` 汇总，再以 `issues_only:true`、path_prefix
细查；默认 8 条，最多 32 条，按 next_offset 翻页，64 KiB 响应预算。
匿名条目保留 XML 位置，组件实例上下文分别保留。未发现 task-ref 需人工判断，
不等于调度错误；引用未解析仅限所选范围，不代表 DaVinci 校验错误。
它尚不展开所有端口实例或证明全部连接完整，也不验证周期/优先级。

`inspect_ecuc_containers` 默认最多 32 项，`limit` 上限 256，单次 JSON 不超过
64 KiB。小结果保留数组格式；超过条目上限会提示 `INSPECTION_PAGE_REQUIRED`。
加 `paged:true` 获取 `count/containers/truncated/next_offset`，按 offset 翻页；
值不会静默截断。大字段请缩小 params。数组批量中的分页结果保留元数据对象。

Before guessing fields, run `lgk-autosar.exe --describe locate_container`.
This prints the request contract without a project, Host or DaVinci startup.
`locate_container` requires `module` and `definition_ref`; `module_name`
remains an alias. Filter instances with `short_name_regex`, not `query` or
`path`. Invalid fields are reported together with a correct example.
Results default to 32 containers (maximum 256); `count` is the full match
count. Continue with `offset: next_offset` only when `truncated` is true.
Prefer exact names and needed parameters to full-module dumps. Reuse a Host
for related requests and close it once at the end. Use the current installed
CLI/Host pair; do not restore an old ZIP when an entry path is missing.
