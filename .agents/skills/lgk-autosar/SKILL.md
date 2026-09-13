---
name: lgk-autosar
description: Inspect and operate local Vector DaVinci ECUC projects through LGK-AUTOSAR.
---

# LGK-AUTOSAR

Read the repository-root `AGENTS.md` and `SKILL.md` before operating a Vector
DaVinci ECUC project. Follow their request workflow, keep changes scoped,
validate results, and always issue `shutdown_host` at the end.

Use `scripts/Invoke-LGKAutosar.ps1` as the normal interface. Do not manually
edit ECUC ARXML when a verified LGK-AUTOSAR request can make the intended
DaVinci configuration change.
