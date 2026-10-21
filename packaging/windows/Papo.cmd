@echo off
rem EN-US: DLL search paths must be set before the executable starts.
rem PT-BR: Caminhos das DLLs precisam ser definidos antes de iniciar o executavel.
setlocal
if not defined PAPO_RUNTIME_DIR set "PAPO_RUNTIME_DIR=C:\msys64\ucrt64"
if not exist "%PAPO_RUNTIME_DIR%\bin\libgtk-4-1.dll" (
  echo MSYS2 UCRT64 runtime not found. Set PAPO_RUNTIME_DIR to your UCRT64 directory.
  pause
  exit /b 1
)
set "PATH=%PAPO_RUNTIME_DIR%\bin;%PATH%"
"%~dp0papo-gtk.exe" %*
exit /b %errorlevel%
