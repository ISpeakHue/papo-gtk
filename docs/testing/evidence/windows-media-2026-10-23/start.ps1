$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'PAPO-CLEAN-VM' -or $env:USERNAME -ne 'PapoTester'){throw 'Disposable fixture only'}
$disk=Get-Volume|Where-Object{$_.DriveLetter -and (Test-Path ($_.DriveLetter+':\PAPO_RESULTS.marker'))}|Select-Object -First 1
if(!$disk){throw 'Missing result disk'}
$output=$disk.DriveLetter+':\results';New-Item -ItemType Directory -Force $output|Out-Null
$source=$PSScriptRoot;$archive=(Get-ChildItem $source -Filter '*.zip'|Select-Object -First 1).FullName
$report=[ordered]@{schema=1;started_utc=[DateTime]::UtcNow.ToString('o');status='running';child_exits=@()}
try{
 & powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$source\check-media-ui.ps1" -Archive $archive -OutputDirectory "$output\cairo" -Renderer cairo *> "$output\cairo-driver.log"
 $report.child_exits+=@{name='cairo';exit_code=$LASTEXITCODE}
 if(@($report.child_exits|Where-Object exit_code -ne 0).Count){throw 'Packaged media acceptance failed'}
 $report.status='pass'
}catch{$report.status='fail';$report.error=$_.Exception.Message}
finally{
 $report.finished_utc=[DateTime]::UtcNow.ToString('o');$report|ConvertTo-Json -Depth 8|Set-Content "$output\VM-RESULT.json" -Encoding UTF8
 shutdown.exe /s /t 0
}
