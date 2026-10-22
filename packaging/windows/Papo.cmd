@echo off
rem EN-US: The native bootstrap finds the adjacent runtime before GTK starts.
rem PT-BR: O iniciador nativo encontra o runtime ao lado antes de iniciar o GTK.
"%~dp0Papo.com" %*
exit /b %errorlevel%
