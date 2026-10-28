# EN-US: Run step 4 in the offline fixture; always flush its evidence by shutting down.
# PT-BR: Executa a etapa 4 no teste offline e sempre encerra para gravar as evidências.
$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'PAPO-CLEAN-VM' -or $env:USERNAME -ne 'PapoTester'){throw 'Disposable fixture account required'}
# EN-US: This guarded offline account runs fixture PowerShell only; stop both old wrapper and child.
# PT-BR: Esta conta offline protegida só executa PowerShell de teste; encerra wrapper e filho antigos.
Get-CimInstance Win32_Process -Filter "Name='powershell.exe'" | Where-Object {$_.ProcessId -ne $PID} | ForEach-Object {Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue}
Remove-Item "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\Startup\PapoAcceptance.cmd" -ErrorAction SilentlyContinue
$volume=[IO.DriveInfo]::GetDrives()|Where-Object {$_.IsReady -and (Test-Path (Join-Path $_.RootDirectory.FullName 'PAPO_RESULTS.marker'))}|Select-Object -First 1
$output=Join-Path $volume.RootDirectory.FullName 'results'
New-Item -ItemType Directory -Force $output|Out-Null
$archive=(Get-ChildItem $PSScriptRoot -Filter 'Papo-*-windows-x86_64-dev.zip'|Select-Object -First 1).FullName
try{& powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'check-identity.ps1') -Archive $archive -OutputDirectory $output -ExternalClicks *> (Join-Path $output 'identity-driver.log');$LASTEXITCODE|Set-Content (Join-Path $output 'identity-child-exit.txt')}
finally{shutdown /s /t 0}
