$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'PAPO-CLEAN-VM' -or $env:USERNAME -ne 'PapoTester'){throw 'Disposable fixture only'}
$source=(Get-Volume|Where-Object FileSystemLabel -eq 'PAPOMEDIA7').DriveLetter+':\'
$result=(Get-Volume|Where-Object FileSystemLabel -eq 'PAPORESULTS').DriveLetter+':\diagnostic'
$archive=(Get-ChildItem $source -Filter '*.zip'|Select-Object -First 1).FullName
& "$PSScriptRoot\check-media-ui.ps1" -Archive $archive -TestDirectory $source -OutputDirectory $result -Renderer auto -Cases @('media-emojis','mov-media')
