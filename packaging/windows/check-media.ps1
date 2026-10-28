# EN-US: Explicit standard-user packaged decoder isolation acceptance, with synthetic input only.
# PT-BR: Aceitação explícita do isolamento dos decodificadores como usuário padrão, só com entrada sintética.
param([Parameter(Mandatory=$true)][string]$Archive,[Parameter(Mandatory=$true)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
New-Item -ItemType Directory -Force $OutputDirectory|Out-Null
$OutputDirectory=(Get-Item $OutputDirectory).FullName
$report=[ordered]@{schema=1;status='running';started_utc=[DateTime]::UtcNow.ToString('o')}
$work=$null;$process=$null
try {
 $identity=[Security.Principal.WindowsIdentity]::GetCurrent();$principal=New-Object Security.Principal.WindowsPrincipal($identity)
 if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator) -or @($identity.Groups|ForEach-Object Value) -contains 'S-1-5-32-544'){throw 'Use a disposable standard account'}
 if(Get-Process -Name papo-gtk -ErrorAction SilentlyContinue){throw 'Close other Papo instances before explicit isolation testing'}
 $report.user=@{name=$identity.Name;administrator=$false}
 $os=Get-CimInstance Win32_OperatingSystem;$version=Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
 $report.os=@{caption=$os.Caption;version=$os.Version;display_version=$version.DisplayVersion;build=$version.CurrentBuildNumber;ubr=$version.UBR;edition=$version.EditionID}
 $file=Get-Item $Archive;$hash=(Get-FileHash $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
 if($hash -ne ([IO.File]::ReadAllText($file.FullName+'.sha256')).Split(' ')[0]){throw 'ZIP checksum mismatch'}
 $report.archive=@{name=$file.Name;sha256=$hash}
 $work=Join-Path $env:USERPROFILE ('WM'+[guid]::NewGuid().ToString('N').Substring(0,6));New-Item -ItemType Directory $work|Out-Null
 $accents=[string][char]0xe1+[char]0xe9+[char]0xe7;$destination=Join-Path $work ('Papo '+$accents)
 Add-Type -AssemblyName System.IO.Compression.FileSystem;[IO.Compression.ZipFile]::ExtractToDirectory($file.FullName,$destination)
 $package=(Get-ChildItem $destination -Directory|Select-Object -First 1).FullName
 $build=[IO.File]::ReadAllText((Join-Path $package 'BUILD-INFO.json'))|ConvertFrom-Json
 if($build.source_dirty){throw 'Expected clean-source package'}
 $report.source_commit=$build.source.commit;$report.runtime_manifest_sha256=$build.runtime.manifest_sha256
 Get-ChildItem Env:|Where-Object Name -Match '^(PAPO_|PYTHON|GST_|GI_|GIO_|XDG_)'|ForEach-Object{Remove-Item "Env:$($_.Name)"}
 $env:PATH="$env:SystemRoot\System32;$env:SystemRoot;$env:SystemRoot\System32\WindowsPowerShell\v1.0"
 $beforePath=[Environment]::GetEnvironmentVariable('PATH','Machine')
 $process=Start-Process (Join-Path $package 'Papo.com') -ArgumentList '--check-media-runtime' -WorkingDirectory $env:SystemRoot -NoNewWindow -PassThru -RedirectStandardOutput (Join-Path $OutputDirectory 'media.stdout.txt') -RedirectStandardError (Join-Path $OutputDirectory 'media.stderr.txt')
 $null=$process.Handle
 if(!$process.WaitForExit(240000)){$process.Kill();throw 'Isolation acceptance timed out'};$process.WaitForExit();$report.exit_code=$process.ExitCode
 if($process.ExitCode -ne 0){throw 'Isolation acceptance failed; see native stderr'}
 $lines=([IO.File]::ReadAllText((Join-Path $OutputDirectory 'media.stdout.txt'),[Text.Encoding]::UTF8)).Split("`n")
 $probe=($lines|Where-Object{$_.StartsWith('{')}|Select-Object -Last 1)|ConvertFrom-Json
 if($probe.status -ne 'pass' -or $probe.posters -ne 3 -or $probe.animations -ne 3 -or $probe.isolation.backend -ne 'appcontainer_job'){throw 'Native isolation report incomplete'}
 $report.probe=$probe
 $manifest=[IO.File]::ReadAllText((Join-Path $package 'runtime/RUNTIME-MANIFEST.json'))|ConvertFrom-Json
 foreach($entry in $manifest.files){if((Get-FileHash (Join-Path $package $entry.path) -Algorithm SHA256).Hash -ne $entry.sha256){throw 'Runtime contents changed'}}
 $report.immutable_runtime=$true;$report.machine_path_unchanged=($beforePath -eq [Environment]::GetEnvironmentVariable('PATH','Machine'))
 if(!$report.machine_path_unchanged){throw 'Machine PATH changed'}
 $report.status='pass'
}catch{$report.status='fail';$report.error=$_.Exception.Message;$_|Out-String|Set-Content (Join-Path $OutputDirectory 'media.failure.txt') -Encoding UTF8}
finally {
 if($process -and !$process.HasExited){$process.Kill()}
 if($work){try{Remove-Item $work -Recurse -Force;$report.extracted_package_removed=$true}catch{$report.status='fail';$report.cleanup_error=$_.Exception.Message}}
 $report.finished_utc=[DateTime]::UtcNow.ToString('o');$report|ConvertTo-Json -Depth 8|Set-Content (Join-Path $OutputDirectory 'media.json') -Encoding UTF8
}
if($report.status -ne 'pass'){exit 1}
