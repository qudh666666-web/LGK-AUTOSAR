---
name: lgk-vector
description: Deprecated LGK-VECTOR entry. Migrate all new work to LGK-AUTOSAR at D:/Tools/LGK-AUTOSAR.
---

# LGK-VECTOR 即将弃用，请迁移至 LGK-AUTOSAR

自 2026-09-14 起，LGK-VECTOR 不再作为新的任务入口，后续维护统一迁移至 LGK-AUTOSAR。

- 新工具目录：`D:\Tools\LGK-AUTOSAR`
- 新调用入口：`D:\Tools\LGK-AUTOSAR\scripts\Invoke-LGKAutosar.ps1`
- 新使用说明：`D:\Tools\LGK-AUTOSAR\SKILL.md`
- 云端仓库：https://github.com/qudh666666-web/LGK-AUTOSAR

任何 Agent、脚本或维护者看到本说明后，请改用新入口。不要从旧 ZIP 恢复旧工具，不要继续修补或构建此目录。新工具兼容读取现有 lgk-vector.json，不必为了改名重建 ECUC 工程。

旧 PowerShell 入口仅允许 shutdown_host 清理已有会话，其他请求会返回迁移提示，不会自动执行或转发。历史 EXE 直接运行不会加载本说明，请更新直接引用旧 EXE 的自动化配置。

示例：

```powershell
& 'D:\Tools\LGK-AUTOSAR\scripts\Invoke-LGKAutosar.ps1' -ProjectPath '<实际 Cfg 目录>' -Request '{"func":"find_module","module":"Can"}'
```