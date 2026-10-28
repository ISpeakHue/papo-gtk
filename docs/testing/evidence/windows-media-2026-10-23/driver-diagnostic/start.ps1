$ErrorActionPreference='Stop'
$result=(Get-Volume|Where-Object FileSystemLabel -eq 'PAPORESULTS').DriveLetter+':\'
try{& powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\probe.ps1" *> "$result\diagnostic-driver.log";$LASTEXITCODE|Set-Content "$result\diagnostic-exit.txt"}finally{shutdown.exe /s /t 0}
