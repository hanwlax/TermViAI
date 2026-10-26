@echo off
setlocal
if not exist "%~dp0TermViAI.exe" (
  echo Extract the entire trial ZIP before running this launcher.
  pause
  exit /b 1
)
set "TERMVIAI_RESIZE_TRACE=%~dp0resize-trace.csv"
echo Resize timings will be appended to "%TERMVIAI_RESIZE_TRACE%"
start "" "%~dp0TermViAI.exe" start --always-new-process --no-auto-connect
endlocal
