# EN-US: Run step 4 in the offline fixture; always flush its evidence by shutting down.
# PT-BR: Executa a etapa 4 no teste offline e sempre encerra para gravar as evidências.
$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'PAPO-CLEAN-VM' -or $env:USERNAME -ne 'PapoTester'){throw 'Disposable fixture account required'}
Get-CimInstance Win32_Process -Filter "Name='powershell.exe'" | Where-Object {$_.ProcessId -ne $PID -and $_.CommandLine -like '*C:\PapoAcceptance\*.ps1*'} | ForEach-Object {Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue}
$volume=[IO.DriveInfo]::GetDrives()|Where-Object {$_.IsReady -and (Test-Path (Join-Path $_.RootDirectory.FullName 'PAPO_RESULTS.marker'))}|Select-Object -First 1
$output=Join-Path $volume.RootDirectory.FullName 'results'
$archive=(Get-ChildItem $PSScriptRoot -Filter 'Papo-*-windows-x86_64-dev.zip'|Select-Object -First 1).FullName
try{& powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'check-identity.ps1') -Archive $archive -OutputDirectory $output}
finally{shutdown /s /t 0}
