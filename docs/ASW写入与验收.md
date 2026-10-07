# ASW 写入与验收

2026-10-07：正式接口 `write_asw_bundle` 已实现一个有边界的 ASW 创建、更新、删除闭环。
写入保存的 ARXML，然后由 DaVinci 重新加载；它不是 DaVinci 内存模型的原生创建 API。
本机许可匹配的非客户 SIP 示例副本已完成真实加载、导出及保存验收，范围见下文。

## 支持范围

| 输入 | 生成的 AUTOSAR 对象 | 范围 |
| --- | --- | --- |
| `types` | IMPLEMENTATION-DATA-TYPE | scalar、alias、array、record；引用已有 Base Type 或 Implementation Type |
| `interfaces` | Sender/Receiver、Client/Server Interface | 数据元素、操作、IN/OUT/INOUT 参数；非 service 接口 |
| `components` | Application SWC、P/R Port | Interface 引用；本文件中定义的 Runnable、Internal Behavior |
| Runnable | Runnable Entity、Timing Event、Variable Access | C 符号名；可选周期；本文件 SR Interface 的读/写访问 |
| `compositions` | Composition、实例、Assembly/Delegation Connector | 实例只引用本文件 Application SWC；方向和接口一致性检查 |

尚不支持 Application Data Type/Data Type Mapping Set、枚举、ComSpec/初值/队列、
Server Event/Call Point、Mode Switch、嵌套或外部 SWC 实例、SWC Implementation/C 源码生成。
ECU 分配、Data 到 Signal 映射、RTE–OS 任务映射及 RTE 代码生成也不在本接口验收范围。
已有映射查询继续使用 `inspect_autosar_mapping`，不能将查询能力当成写入能力。

## 文件与请求约束

目标必须是工程内 DPA 的 `Folders/ApplicationComponentFolders/ApplicationComponentFolder`
已登记目录中的绝对 `.arxml` 路径，父目录须存在。单纯放进 `Config/Developer` 不保证
DaVinci 会加载；只有该目录也被登记为应用输入时才可写入。接口不替用户修改 DPA。
禁止工程外目录、路径穿越、链接文件及 ECUC/System/ServiceComponents/AUTOSAR/
InternalBehavior 配置目录。任何登记的应用输入在工程外或无法访问时，写入拒绝执行，
因为不能完整复核其依赖；只读查询仍只扫描可访问的工程内输入。

查询 `scope:model/developer/all` 和 `set_asw_reference` 已包含这些工程内登记输入。
扫描仍受 512 个文件、单文件 64 MiB、总量 256 MiB 的预算约束，不查询工程外材料。

每个 Bundle 独占一个顶层 Package 和一个带 LGK 标记的文件。创建必须传 `expected:null`，
目标已存在便拒绝，不覆盖已有 ASW。更新/删除必须传该文件的**完整 UTF-8 旧内容**，
逐字节复核；只接受 LGK 创建且包含单个 Package 的文件，不能改 Package 名。
更新根据完整声明重写该专用文件；不要手工加入需要保留的 UUID、描述或额外建模内容。
它不是任意既有 ASW 文件的保留式编辑器。

单文件最大 256 KiB，请求最大 512 KiB，最多 128 个顶层对象及 512 个命名对象。
名称/C 符号采用 1–128 字节 ASCII 标识符；数组长度 1–65535；周期单位为秒，须有限且
大于 0、不超过 86400。未知字段、重复语义路径、缺失/歧义目标、DEST 不符、类型环、
连接方向或接口不符均拒绝。其他已扫描文件仍引用的对象，不可删除、改类型或改定义；
更新同文件中其他未被外部引用的对象仍可执行。不提供强制覆盖选项。

创建通过同目录临时文件、同步和原子无覆盖发布提交；文件系统不支持硬链接则报错，
没有覆盖回退。更新沿用旧字节复核及原子替换；删除先复核旧字节再移除文件。
这些操作不是跨文件事务或持久撤销。调用前保存并关闭同工程 GUI，顺序发送独立请求；
已有 LGK DaVinci 会话时先 `shutdown_host`。依赖文件不被整体锁定，禁止并发编辑。

## 使用示例

先查询目标工程中已有 Implementation Type，再使用其实际语义路径。下面的 `uint8`
仅是示例，不保证所有工程都有此定义。目标文件应置于已经登记的应用输入目录。

```json
{
  "func": "write_asw_bundle",
  "file": "D:\\Work\\Vehicle\\Cfg\\Config\\ApplicationComponents\\Authored.arxml",
  "expected": null,
  "preview": true,
  "bundle": {
    "package": "Authored",
    "types": [{"kind":"alias","name":"Counter","type_ref":"/AUTOSAR_Platform/ImplementationDataTypes/uint8"}],
    "interfaces": [{"kind":"sender_receiver","name":"Values","data_elements":[{"name":"Count","type_ref":"/Authored/Counter"}]}],
    "components": [{
      "name":"Producer",
      "ports":[{"name":"Out","direction":"provide","interface_ref":"/Authored/Values"}],
      "runnables":[{"name":"Tick","symbol":"Producer_Tick","period_seconds":0.01,"writes":[{"port":"Out","data_element":"Count"}]}]
    }]
  }
}
```

预览成功后以相同声明单独发送 `preview:false`。读写访问的字段为 `port` 和
`data_element`；`reads` 对应 R Port，`writes` 对应 P Port。Client/Server 的
`operations` 内有 `name`、`arguments`，参数字段为 `name/type_ref/direction`。

Composition 示例；此处 Producer/Consumer 均须在同一个 Bundle 的 components 中声明：

```json
{
  "name":"Root",
  "instances":[{"name":"Tx","type_ref":"/Authored/Producer"},{"name":"Rx","type_ref":"/Authored/Consumer"}],
  "ports":[{"name":"Out","direction":"provide","interface_ref":"/Authored/Values"}],
  "connections":[{"name":"Link","provider":{"instance":"Tx","port":"Out"},"requester":{"instance":"Rx","port":"In"}}],
  "delegations":[{"name":"Expose","inner":{"instance":"Tx","port":"Out"},"outer_port":"Out"}]
}
```

更新时替换 `bundle` 的完整声明，并在本地读取旧内容构造请求；不必将整文件塞进 Agent
对话。删除时不给 bundle，使用 `delete:true`，同样先预览：

```powershell
$old = [Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes($file))
$request = @{func='write_asw_bundle';file=$file;expected=$old;delete=$true;preview=$true}
& '.\scripts\Invoke-LGKAutosar.ps1' -ProjectPath $project `
  -Request ($request | ConvertTo-Json -Depth 15 -Compress)
```

结果返回 operation、preview、changed、applied、deleted、对象数和前后字节数。
预览不设置 applied/deleted。`write_path:saved_arxml`、`davinci_validated:false`、
`cross_request_undo_available:false` 明确本次请求不自动启动 DaVinci，也没有跨请求撤销。
写后先重新 inspect/trace，再运行目标工程所需的 DaVinci 校验/生成。

## 验收依据与实测边界

模型关系参考 AUTOSAR 的
[SWC Modeling Guide](https://www.autosar.org/fileadmin/standards/R4.3.1/CP/AUTOSAR_TR_SWCModelingGuide.pdf)，
原生验收 API 以本机 DaVinci Configurator 5 的 AutomationInterface Javadoc 为准。
验收脚本独立编写，没有复用 AutoC 代码或资源，也没有将 Vector 模型/XSD 收入仓库。

维护者入口为 `scripts/Invoke-LGKAswAcceptance.ps1`，原生任务为
`assets/experiments/LGKAswAcceptance.dvgroovy`。两者均排除出用户包。
驱动先调用 Boolean 实验的 PrepareOnly，建立全新、非客户副本及独立 1 GiB 启动配置，
保留 1800 MiB 私有内存保护和 120 秒阶段上限；不会修改原启动 INI。

真实样本为 SIP 自带 StartApplication 的可丢弃副本，DaVinci 版本 5.19.46 SP2。
使用正式包装器创建含四种 Implementation Type、SR/CS Interface、两个 SWC、Port、
Runnable、数据访问、Composition、Assembly/Delegation 的文件；再更新周期，最后删除。
每次关闭 LGK Host 后启动 DaVinci，通过 `mdfModel` 核对对象及原生 MI 类型，
在 `persistency { modelExport { ... } }` 中导出内存模型，复核周期，并 `saveProject()`。

最终证据位于仓库外 `ReverseEngineering/lgk-asw-probe-20261007-3/asw-evidence.json`：

| 阶段 | 原生结果 | 耗时 | ERROR 计数 |
| --- | --- | --- | --- |
| 创建 | 15 个关键对象/类型通过，导出周期 0.01 秒 | 18.1 秒 | 254 |
| 更新 | 同样 15 个对象/类型通过，导出周期 0.02 秒 | 17.6 秒 | 254 |
| 删除 | 重新加载后包不存在 | 18.7 秒 | 254 |

导出同时确认四种 Implementation Type、Record/Array 元素、CS 操作/参数、两个数据访问及
两个 Connector。506 个源示例文件及原 INI 哈希不变。此前 -2 实验因 export 调用缺少
persistency 委托而失败；修正后在全新 -3 副本完成上述三阶段，失败日志保留。

ERROR 计数相同不证明零错误或全部诊断内容相同。实验未执行 RTE 生成、ECU 分配、
OS 任务配置或应用编译，也没有验证 DaVinci Developer GUI。DaVinci 加载/保存可能修改
副本的 DPA/Os 等文件，删除本次 ASW 文件不能称整工程恢复；跨请求撤销仍待独立实现。
公开 Rust/onboarding 测试使用合成模型，覆盖静态校验、旧值、依赖保护、预览及包边界；
其成功与上述真实原生加载证据分别记录，不互相替代。
