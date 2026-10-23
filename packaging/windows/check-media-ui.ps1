# EN-US: Opt-in packaged UI acceptance; synthetic loopback fixtures and isolated settings only.
# PT-BR: Aceitação opcional da interface empacotada; só fixtures locais sintéticas e configurações isoladas.
param(
 [Parameter(Mandatory=$true)][string]$Archive,
 [Parameter(Mandatory=$true)][string]$OutputDirectory,
 [string]$TestDirectory=$PSScriptRoot,
 [ValidateSet('cairo','auto')][string]$Renderer='cairo',
 [ValidateSet('discard','device')][string]$AudioOutput='discard',
 [string[]]$Cases=@('media-emojis','draft-previews','attachment-media','mov-media','backend-gifs','media-cache','recent-media','status-viewer','send-video','video-audio')
)
$ErrorActionPreference='Stop'
New-Item -ItemType Directory -Force $OutputDirectory|Out-Null
$OutputDirectory=(Get-Item $OutputDirectory).FullName
$report=[ordered]@{schema=1;status='running';started_utc=[DateTime]::UtcNow.ToString('o');renderer_request=$Renderer;audio_output=$AudioOutput;cases=@()}
$work=$null;$process=$null
try {
 if(Get-Process -Name papo-gtk,papo-media-tests -ErrorAction SilentlyContinue){throw 'Close Papo before the explicit disposable media UI check'}
 $identity=[Security.Principal.WindowsIdentity]::GetCurrent();$principal=New-Object Security.Principal.WindowsPrincipal($identity)
 $report.user=@{name=$identity.Name;administrator=$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)}
 $os=Get-CimInstance Win32_OperatingSystem;$version=Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
 $report.os=@{caption=$os.Caption;version=$os.Version;display_version=$version.DisplayVersion;build=$version.CurrentBuildNumber;ubr=$version.UBR;edition=$version.EditionID}
 $file=Get-Item $Archive;$hash=(Get-FileHash $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
 if($hash -ne ([IO.File]::ReadAllText($file.FullName+'.sha256')).Split(' ')[0].Trim()){throw 'ZIP checksum mismatch'}
 $metadata=[IO.File]::ReadAllText((Join-Path $TestDirectory 'MEDIA-TESTS.json'))|ConvertFrom-Json
 $test=Join-Path $TestDirectory 'papo-media-tests.exe'
 if((Get-FileHash $test -Algorithm SHA256).Hash -ne $metadata.sha256){throw 'UI executable checksum mismatch'}
 $report.archive=@{name=$file.Name;sha256=$hash};$report.instrumented_ui=$metadata
 $work=Join-Path $env:USERPROFILE ('MU'+[guid]::NewGuid().ToString('N').Substring(0,6));New-Item -ItemType Directory $work|Out-Null
 $accents=[string][char]0xe1+[char]0xe9+[char]0xe7;$destination=Join-Path $work ('Papo '+$accents)
 Add-Type -AssemblyName System.IO.Compression.FileSystem;[IO.Compression.ZipFile]::ExtractToDirectory($file.FullName,$destination)
 $package=(Get-ChildItem $destination -Directory|Select-Object -First 1).FullName
 $build=[IO.File]::ReadAllText((Join-Path $package 'BUILD-INFO.json'))|ConvertFrom-Json
 if($build.source_dirty -or $metadata.source_dirty -or $metadata.source_commit -ne $build.source.commit){throw 'UI test and package source must match and be clean'}
 $report.source_commit=$build.source.commit;$report.runtime_manifest_sha256=$build.runtime.manifest_sha256
 $runtime=Join-Path $package 'runtime';$runner=Join-Path $runtime 'bin/papo-media-tests.exe';Copy-Item $test $runner
 # EN-US: Mirror the launcher's private DLL/module paths; do not expose MSYS2 or change system settings.
 # PT-BR: Replica caminhos privados de DLLs/módulos do launcher; não expõe MSYS2 nem altera ajustes do sistema.
 Get-ChildItem Env:|Where-Object Name -Match '^(PAPO_|PYTHON|GST_|GI_|GTK_|GSK_|GDK_|GIO_|GSETTINGS_SCHEMA_DIR|FONTCONFIG_|XDG_)'|ForEach-Object{Remove-Item "Env:$($_.Name)"}
 $env:PATH="$runtime\bin;$env:SystemRoot\System32;$env:SystemRoot;$env:SystemRoot\System32\WindowsPowerShell\v1.0"
 $env:PAPO_RUNTIME_DIR=$runtime;$env:PYTHONHOME=$runtime;$env:PYTHONNOUSERSITE='1';$env:PYTHONSAFEPATH='1';$env:PYTHONDONTWRITEBYTECODE='1'
 $env:GI_TYPELIB_PATH=Join-Path $runtime 'lib/girepository-1.0';$env:GSETTINGS_SCHEMA_DIR=Join-Path $runtime 'share/glib-2.0/schemas'
 $env:GTK_PATH=Join-Path $runtime 'lib/gtk-4.0';$env:GIO_MODULE_DIR=Join-Path $runtime 'lib/gio/modules'
 $env:XDG_DATA_DIRS=Join-Path $runtime 'share';$env:FONTCONFIG_PATH=Join-Path $runtime 'etc/fonts';$env:FONTCONFIG_FILE=Join-Path $runtime 'etc/fonts/fonts.conf'
 $cache=Join-Path $work 'cache';New-Item -ItemType Directory $cache|Out-Null;$env:XDG_CACHE_HOME=$cache
 $env:GST_PLUGIN_SYSTEM_PATH_1_0=Join-Path $runtime 'lib/gstreamer-1.0';$env:GST_PLUGIN_SCANNER_1_0=Join-Path $runtime 'libexec/gstreamer-1.0/gst-plugin-scanner.exe';$env:GST_REGISTRY_1_0=Join-Path $cache 'registry.bin'
 $loaders=Get-ChildItem (Join-Path $runtime 'lib/gdk-pixbuf-2.0') -Filter loaders.cache -Recurse|Select-Object -First 1
 if($loaders){$env:GDK_PIXBUF_MODULE_FILE=$loaders.FullName}
 # EN-US: Decode audio into the bundled test sink on device-free runners; never claim audible output.
 # PT-BR: Decodifica áudio no sink de teste distribuído em runners sem dispositivo; não comprova saída audível.
 if($AudioOutput -eq 'discard'){$env:GST_PLUGIN_FEATURE_RANK='fakeaudiosink:512'}
 $env:GTK_A11Y='none';$env:PYTHONUTF8='1';$env:PAPO_NATIVE_FULLSCREEN_TEST='1'
 if($Renderer -eq 'auto'){Remove-Item Env:GSK_RENDERER -ErrorAction SilentlyContinue}else{$env:GSK_RENDERER=$Renderer}
 $beforePath=[Environment]::GetEnvironmentVariable('PATH','Machine')
 foreach($case in $Cases){
  if($case -notin @('media-emojis','draft-previews','attachment-media','mov-media','backend-gifs','media-cache','recent-media','status-viewer','send-video','video-audio')){throw 'Invalid media case name'}
  $env:PAPO_UI_CASE=$case;$begin=[DateTime]::UtcNow
  $stdout=Join-Path $OutputDirectory ($case+'.stdout.txt');$stderr=Join-Path $OutputDirectory ($case+'.stderr.txt')
  $process=Start-Process $runner -ArgumentList @('ui_smoke','--ignored','--test-threads=1','--nocapture') -WorkingDirectory $env:SystemRoot -NoNewWindow -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
  $null=$process.Handle;$peak=0;$timedout=$false
  while(!$process.WaitForExit(250)){
   try{$process.Refresh();if(!$process.HasExited){$peak=[Math]::Max($peak,$process.WorkingSet64)}}catch{if(!$process.HasExited){throw}}
   if(([DateTime]::UtcNow-$begin).TotalSeconds -ge 240){$timedout=$true;break}
  }
  if($timedout){$process.Kill()};$process.WaitForExit()
  $text=[IO.File]::ReadAllText($stdout,[Text.Encoding]::UTF8)
  $pass=!$timedout -and $process.ExitCode -eq 0 -and $text.Contains('1 passed; 0 failed')
  $report.cases+=@{name=$case;status=$(if($pass){'pass'}else{'fail'});exit_code=$process.ExitCode;timed_out=$timedout;seconds=([DateTime]::UtcNow-$begin).TotalSeconds;ui_peak_working_set_bytes=$peak;renderer=($text.Split("`n")|Where-Object{$_ -like '*Papo GUI renderer:*'}|Select-Object -First 1)}
 }
 Remove-Item $runner -Force
 $manifest=[IO.File]::ReadAllText((Join-Path $runtime 'RUNTIME-MANIFEST.json'))|ConvertFrom-Json
 foreach($entry in $manifest.files){if((Get-FileHash (Join-Path $package $entry.path) -Algorithm SHA256).Hash -ne $entry.sha256){throw 'Runtime contents changed'}}
 $report.immutable_runtime=$true;$report.machine_path_unchanged=($beforePath -eq [Environment]::GetEnvironmentVariable('PATH','Machine'))
 if(!$report.machine_path_unchanged -or @($report.cases|Where-Object status -ne 'pass').Count -gt 0){throw 'Media UI acceptance failed; inspect individual case logs'}
 $report.status='pass'
}catch{$report.status='fail';$report.error=$_.Exception.Message;$_|Out-String|Set-Content (Join-Path $OutputDirectory 'media-ui.failure.txt') -Encoding UTF8}
finally{
 if($process -and !$process.HasExited){$process.Kill()}
 if($work){try{Remove-Item $work -Recurse -Force;$report.extracted_package_removed=$true}catch{$report.status='fail';$report.cleanup_error=$_.Exception.Message}}
 $report.finished_utc=[DateTime]::UtcNow.ToString('o');$report|ConvertTo-Json -Depth 8|Set-Content (Join-Path $OutputDirectory ('media-ui-'+$Renderer+'.json')) -Encoding UTF8
}
if($report.status -ne 'pass'){exit 1}
