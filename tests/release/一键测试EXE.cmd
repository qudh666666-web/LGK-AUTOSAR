@echo off
chcp 65001 >nul
title LGK-AUTOSAR EXE Self-Test
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Run-ExeSelfTest.ps1"
set "LGK_TEST_EXIT=%ERRORLEVEL%"
echo.
if not "%LGK_TEST_EXIT%"=="0" (
  echo LGK-AUTOSAR EXE 自检失败。反馈问题时请复制完整输出。
) else (
  echo LGK-AUTOSAR EXE 自检通过。
)
pause
exit /b %LGK_TEST_EXIT%
