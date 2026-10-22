# EN-US: Explicit offline fixture wrapper; flush acceptance evidence by normal shutdown.
# PT-BR: Wrapper explícito de teste offline; grava as evidências pelo encerramento normal.
$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'PAPO-CLEAN-VM' -or $env:USERNAME -ne 'PapoTester'){throw 'Disposable fixture account required'}
Remove-Item "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\Startup\PapoAcceptance.cmd" -ErrorAction SilentlyContinue
$volume=[IO.DriveInfo]::GetDrives()|Where-Object {$_.IsReady -and (Test-Path (Join-Path $_.RootDirectory.FullName 'PAPO_RESULTS.marker'))}|Select-Object -First 1
$output=Join-Path $volume.RootDirectory.FullName 'results';New-Item -ItemType Directory -Force $output|Out-Null
$archive=(Get-ChildItem $PSScriptRoot -Filter 'Papo-*-windows-x86_64-dev.zip'|Select-Object -First 1).FullName
try{& powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'check-storage.ps1') -Archive $archive -OutputDirectory $output *> (Join-Path $output 'storage-driver.log');$LASTEXITCODE|Set-Content (Join-Path $output 'storage-child-exit.txt')}
finally{shutdown /s /t 0}
