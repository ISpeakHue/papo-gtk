# EN-US: Explicit standard-user storage acceptance; never inspect existing accounts or settings.
# PT-BR: Aceitação explícita como usuário padrão; não inspeciona contas nem configurações existentes.
param([Parameter(Mandatory=$true)][string]$Archive,[Parameter(Mandatory=$true)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
New-Item -ItemType Directory -Force $OutputDirectory|Out-Null
$OutputDirectory=(Get-Item $OutputDirectory).FullName
$report=[ordered]@{schema=1;status='running';started_utc=[DateTime]::UtcNow.ToString('o')}
$work=$null;$process=$null
try{
 $identity=[Security.Principal.WindowsIdentity]::GetCurrent();$principal=New-Object Security.Principal.WindowsPrincipal($identity)
 if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator) -or @($identity.Groups|ForEach-Object Value) -contains 'S-1-5-32-544'){throw 'Use a disposable standard account'}
 if(Get-Process -Name papo-gtk -ErrorAction SilentlyContinue){throw 'Close other Papo instances before explicit storage testing'}
 $report.user=@{name=$identity.Name;administrator=$false}
 $os=Get-CimInstance Win32_OperatingSystem;$version=Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
 $report.os=@{caption=$os.Caption;version=$os.Version;display_version=$version.DisplayVersion;build=$version.CurrentBuildNumber;ubr=$version.UBR;edition=$version.EditionID}
 $file=Get-Item $Archive;$hash=(Get-FileHash $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
 if($hash -ne ([IO.File]::ReadAllText($file.FullName+'.sha256')).Split(' ')[0]){throw 'ZIP checksum mismatch'}
 $report.archive=@{name=$file.Name;sha256=$hash}
 $work=Join-Path $env:USERPROFILE ('WS'+[guid]::NewGuid().ToString('N').Substring(0,6));New-Item -ItemType Directory $work|Out-Null
 $accents=[string][char]0xe1+[char]0xe9+[char]0xe7;$destination=Join-Path $work ('Papo '+$accents)
 Add-Type -AssemblyName System.IO.Compression.FileSystem;[IO.Compression.ZipFile]::ExtractToDirectory($file.FullName,$destination)
 $package=(Get-ChildItem $destination -Directory|Select-Object -First 1).FullName
 $build=[IO.File]::ReadAllText((Join-Path $package 'BUILD-INFO.json'))|ConvertFrom-Json
 if($build.source_dirty){throw 'Expected clean-source package'}
 $report.source_commit=$build.source.commit;$report.runtime_manifest_sha256=$build.runtime.manifest_sha256
 $env:GSK_RENDERER='cairo';$env:GTK_A11Y='none'
 $process=Start-Process (Join-Path $package 'Papo.com') -ArgumentList '--check-windows-storage' -WorkingDirectory $env:SystemRoot -NoNewWindow -PassThru -RedirectStandardOutput (Join-Path $OutputDirectory 'storage.stdout.json') -RedirectStandardError (Join-Path $OutputDirectory 'storage.stderr.txt')
 $null=$process.Handle
 if(!$process.WaitForExit(240000)){$process.Kill();throw 'Storage acceptance timed out'};$process.WaitForExit();$report.exit_code=$process.ExitCode
 if($process.ExitCode -ne 0){throw 'Storage acceptance process failed; see stderr'}
 $probe=[IO.File]::ReadAllText((Join-Path $OutputDirectory 'storage.stdout.json'),[Text.Encoding]::UTF8)|ConvertFrom-Json
 if($probe.status -ne 'pass' -or !$probe.scratch_removed -or !$probe.credentials.removed){throw 'Storage acceptance or cleanup did not pass'}
 $report.probe=$probe;$report.status='pass'
}catch{$report.status='fail';$report.error=$_.Exception.Message;$_|Out-String|Set-Content (Join-Path $OutputDirectory 'failure.txt') -Encoding UTF8}
finally{
 if($process -and !$process.HasExited){$process.Kill()}
 if($work){try{Remove-Item $work -Recurse -Force;$report.extracted_package_removed=$true}catch{$report.status='fail';$report.cleanup_error=$_.Exception.Message}}
 $report.finished_utc=[DateTime]::UtcNow.ToString('o');$report|ConvertTo-Json -Depth 8|Set-Content (Join-Path $OutputDirectory 'storage.json') -Encoding UTF8
}
if($report.status -ne 'pass'){exit 1}
