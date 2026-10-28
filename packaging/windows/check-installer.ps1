# EN-US: Opt-in install/upgrade/uninstall checks for a disposable Windows account only.
# PT-BR: Testes explícitos de instalação/atualização/desinstalação só em conta Windows descartável.
param(
 [Parameter(Mandatory=$true)][string]$Installer,
 [Parameter(Mandatory=$true)][string]$OutputDirectory,
 [switch]$DisposableAccount,
 [switch]$RequireSignatures
)
$ErrorActionPreference='Stop'
if(!$DisposableAccount){throw 'Use -DisposableAccount only in an empty disposable Windows account'}
New-Item -ItemType Directory -Force $OutputDirectory|Out-Null
$OutputDirectory=(Get-Item $OutputDirectory).FullName
$report=[ordered]@{schema=1;status='running';cases=@();started_utc=[DateTime]::UtcNow.ToString('o')}
$state='HKCU:\Software\Papo\Installer';$arp='HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\PapoGtk'
$root=Join-Path $env:LOCALAPPDATA 'Programs/Papo';$shortcut=Join-Path ([Environment]::GetFolderPath('Programs')) 'Papo.lnk'
$process=$null;$guard=$null;$markers=@();$credentialCreated=$false
$signingThumbprint=$env:PAPO_SIGNING_THUMBPRINT;$signTool=$env:PAPO_SIGNTOOL
function Record([string]$name){$report.cases+=@{name=$name;status='pass';timed_out=$false;exit_code=0}}
function Run-Setup([string]$path,[string]$arguments='/S',[bool]$expectFailure=$false){
 $script:process=Start-Process -FilePath $path -ArgumentList $arguments -PassThru
 $null=$process.Handle
 if(!$process.WaitForExit(420000)){& "$env:SystemRoot/System32/taskkill.exe" /PID $process.Id /T /F|Out-Null;throw 'Installer process timed out'}
 $process.WaitForExit();$ok=$process.ExitCode -eq 0
 if($ok -eq $expectFailure){throw "Unexpected installer result: $($process.ExitCode)"}
}
function Current-Slot {
 $slot=(Get-ItemProperty $state).CurrentPath
 if(!$slot -or [IO.Path]::GetDirectoryName($slot) -ne (Join-Path $root 'versions')){throw 'Installed path escapes the fixed per-user root'}
 return $slot
}
function Check-Installed([string]$slot){
 if($RequireSignatures){
  $env:PAPO_SIGNING_THUMBPRINT=$signingThumbprint;$env:PAPO_SIGNTOOL=$signTool
  & (Join-Path $PSScriptRoot 'sign.ps1') -Path (Join-Path $slot 'Uninstall.exe') -Receipt (Join-Path $OutputDirectory 'uninstaller-signature.json') -Role uninstaller -VerifyOnly
  if($LASTEXITCODE -ne 0){throw 'Installed uninstaller signature failed'}
  $signature=Get-Content (Join-Path $OutputDirectory 'uninstaller-signature.json') -Raw -Encoding UTF8|ConvertFrom-Json
  if($signature.sha256 -ne $metadata.uninstaller.sha256 -or $signature.signer_certificate_sha256 -ne $metadata.signer_certificate_sha256){throw 'Embedded uninstaller signature differs from the candidate'}
 }
 if((Get-FileHash (Join-Path $slot 'BUILD-INFO.json') -Algorithm SHA256).Hash -ne $metadata.build_info_sha256){throw 'Installed BUILD-INFO differs from portable ZIP'}
 $manifest=Get-Content (Join-Path $slot 'runtime/RUNTIME-MANIFEST.json') -Raw -Encoding UTF8|ConvertFrom-Json
 foreach($entry in $manifest.files){if((Get-FileHash (Join-Path $slot $entry.path) -Algorithm SHA256).Hash -ne $entry.sha256){throw 'Installed runtime differs from portable ZIP'}}
 & (Join-Path $slot 'Papo.com') --check-packaged-runtime > (Join-Path $OutputDirectory 'installed-runtime.stdout.txt') 2> (Join-Path $OutputDirectory 'installed-runtime.stderr.txt')
 if($LASTEXITCODE -ne 0){throw 'Installed runtime probe failed'}
 & (Join-Path $slot 'Papo.com') --desktop-info > (Join-Path $OutputDirectory 'desktop-info.json')
 if($LASTEXITCODE -ne 0){throw 'Installed shell identity is invalid'}
 $link=(New-Object -ComObject WScript.Shell).CreateShortcut($shortcut)
 if($link.TargetPath -ne (Join-Path $slot 'Papo.exe')){throw 'Start menu points to the wrong GUI executable'}
 $entry=Get-ItemProperty $arp
 if($entry.InstallLocation -ne $slot -or $entry.DisplayVersion -ne $metadata.version -or $entry.UninstallString -ne ('"'+(Join-Path $slot 'Uninstall.exe')+'"')){throw 'Installed-app entry is invalid'}
}
try {
 if((Test-Path $root) -or (Test-Path $state) -or (Test-Path $arp) -or (Test-Path $shortcut) -or (Test-Path 'HKCU:\Software\Classes\papo-notification') -or (Get-Process papo-gtk -ErrorAction SilentlyContinue)){throw 'Account already contains Papo; refusing to alter it'}
 $file=Get-Item $Installer;$hash=(Get-FileHash $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
 if($hash -ne ([IO.File]::ReadAllText($file.FullName+'.sha256')).Split(' ')[0].Trim()){throw 'Installer checksum mismatch'}
 $metadata=Get-Content ($file.FullName+'.json') -Raw -Encoding UTF8|ConvertFrom-Json
 if($metadata.schema -ne 1 -or $metadata.source_dirty -or $metadata.installer.sha256 -ne $hash){throw 'Invalid installer provenance'}
 $report.installer=$metadata.installer;$report.archive=$metadata.archive;$report.source_commit=$metadata.source_commit
 $report.build_info_sha256=$metadata.build_info_sha256;$report.runtime_manifest_sha256=$metadata.runtime_manifest_sha256
 $principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
 $report.administrator=$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
 $report.os=@{version=[Environment]::OSVersion.VersionString;ci_image=$env:ImageOS}
 $beforePath=[Environment]::GetEnvironmentVariable('PATH','Machine')
 # EN-US: The native bootstrap must work without developer Python/GStreamer discovery paths.
 # PT-BR: O bootstrap nativo deve funcionar sem caminhos de descoberta de Python/GStreamer de desenvolvimento.
 Get-ChildItem Env:|Where-Object Name -Match '^(PAPO_|PYTHON|GST_|GI_|GTK_|GSK_|GDK_|GIO_|GSETTINGS_SCHEMA_DIR|FONTCONFIG_|XDG_)'|ForEach-Object{Remove-Item "Env:$($_.Name)"}
 $env:PATH="$env:SystemRoot\System32;$env:SystemRoot;$env:SystemRoot\System32\WindowsPowerShell\v1.0"
 $env:GSK_RENDERER='cairo';$env:GTK_A11Y='none';$env:PYTHONUTF8='1'
 # EN-US: Fixtures never replace real settings and never enumerate saved account credentials.
 # PT-BR: Fixtures nunca substituem ajustes reais nem enumeram credenciais de contas salvas.
 foreach($base in @((Join-Path $env:APPDATA 'papo-gtk'),(Join-Path $env:LOCALAPPDATA 'papo-gtk'))){
  if(Test-Path $base){throw 'Existing Papo data directory; use an empty disposable profile'}
  New-Item -ItemType Directory $base|Out-Null
  $marker=Join-Path $base 'installer-preservation-fixture.txt';[IO.File]::WriteAllText($marker,'synthetic installer fixture');$markers+=$marker
 }
 Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class PapoInstallerCredential {
 [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct Credential {
  public uint Flags, Type; public string TargetName, Comment; public System.Runtime.InteropServices.ComTypes.FILETIME LastWritten;
  public uint BlobSize; public IntPtr Blob; public uint Persist, AttributeCount; public IntPtr Attributes; public string Alias, Username;
 }
 [DllImport("advapi32.dll",EntryPoint="CredWriteW",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool Write(ref Credential value,uint flags);
 [DllImport("advapi32.dll",EntryPoint="CredReadW",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool Read(string name,uint type,uint flags,out IntPtr value);
 [DllImport("advapi32.dll",EntryPoint="CredDeleteW",CharSet=CharSet.Unicode)] public static extern bool Delete(string name,uint type,uint flags);
 [DllImport("advapi32.dll")] static extern void CredFree(IntPtr value);
 public static void Create(string name){var bytes=new byte[]{11,22,33,44};var p=Marshal.AllocHGlobal(bytes.Length);try{Marshal.Copy(bytes,0,p,bytes.Length);var c=new Credential{Type=1,TargetName=name,Username="Papo fixture",Blob=p,BlobSize=4,Persist=2};if(!Write(ref c,0))throw new Exception("Fixture credential write failed");}finally{Marshal.FreeHGlobal(p);}}
 public static bool Preserved(string name){IntPtr p;if(!Read(name,1,0,out p))return false;try{var c=(Credential)Marshal.PtrToStructure(p,typeof(Credential));return c.BlobSize==4&&Marshal.ReadByte(c.Blob)==11&&Marshal.ReadByte(c.Blob,3)==44;}finally{CredFree(p);}}
}
'@
 $credential='br.com.papo.gtk.session.v2:installer-fixture-'+[guid]::NewGuid().ToString('N')
 [PapoInstallerCredential]::Create($credential);$credentialCreated=$true
 Run-Setup $file.FullName
 $first=Current-Slot;Check-Installed $first;Record 'install'
 # EN-US: Same-version replacement exercises the full versioned upgrade transaction, not in-place copying.
 # PT-BR: A substituição da mesma versão testa a transação versionada completa, sem sobrescrever a cópia ativa.
 Run-Setup $file.FullName
 $second=Current-Slot
 if($second -eq $first){throw 'Upgrade overwrote the active slot'}
 Check-Installed $second;Record 'upgrade'
 $foreign=Join-Path $first 'user-created-note.txt';[IO.File]::WriteAllText($foreign,'preserve unknown files')
 Run-Setup (Join-Path $first 'Uninstall.exe') ('/S _?='+$first)
 if((Current-Slot) -ne $second){throw 'Old uninstaller changed the active install'}
 Check-Installed $second
 if(!(Test-Path $foreign)){throw 'Old uninstaller removed an untracked file'}
 Remove-Item (Join-Path $first 'Uninstall.exe') -Force -ErrorAction SilentlyContinue
 Record 'stale-uninstaller'
 # EN-US: A lock must reject uninstall before shell identity or program files change.
 # PT-BR: Um bloqueio deve impedir a desinstalação antes de alterar a identidade ou os arquivos.
 $guard=[IO.File]::Open((Join-Path $second 'runtime/bin/papo-gtk.exe'),[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
 Run-Setup (Join-Path $second 'Uninstall.exe') ('/S _?='+$second) $true
 $guard.Dispose();$guard=$null;Check-Installed $second;Record 'locked-uninstall'
 # EN-US: Recreate the durable interrupted-commit journal, then require automatic recovery on retry.
 # PT-BR: Recria o diário durável de uma confirmação interrompida e exige recuperação automática na repetição.
 Run-Setup $file.FullName
 $interrupted=Current-Slot
 if($interrupted -eq $second){throw 'Interrupted-upgrade fixture needs distinct slots'}
 New-ItemProperty $state -Name PreviousPath -Value $second -PropertyType String -Force|Out-Null
 New-ItemProperty $state -Name PreviousDesktop -Value $second -PropertyType String -Force|Out-Null
 New-ItemProperty $state -Name PendingPath -Value $interrupted -PropertyType String -Force|Out-Null
 Run-Setup $file.FullName '/S /RECOVERONLY'
 if((Current-Slot) -ne $second){throw 'Interrupted upgrade did not restore the previous active path'}
 Check-Installed $second
 Run-Setup $file.FullName
 $third=Current-Slot;Check-Installed $third
 if((Get-ItemProperty $state).PSObject.Properties.Name -contains 'PendingPath'){throw 'Recovery journal was not cleared'}
 Record 'interrupted-recovery'
 Run-Setup (Join-Path $third 'Uninstall.exe') ('/S _?='+$third)
 Remove-Item (Join-Path $third 'Uninstall.exe') -Force -ErrorAction SilentlyContinue
 if((Test-Path $state) -or (Test-Path $arp) -or (Test-Path $shortcut) -or (Test-Path 'HKCU:\Software\Classes\papo-notification')){throw 'Owned shell/app registration survived uninstall'}
 if(Get-ChildItem $root -Filter papo-gtk.exe -Recurse -ErrorAction SilentlyContinue){throw 'Tracked program files survived uninstall'}
 Record 'uninstall'
 foreach($marker in $markers){if([IO.File]::ReadAllText($marker) -ne 'synthetic installer fixture'){throw 'User data was changed'}}
 if(![PapoInstallerCredential]::Preserved($credential)){throw 'Saved fixture credential was removed'}
 if(!(Test-Path $foreign)){throw 'An unknown file was deleted'}
 Record 'data-preserved'
 $report.machine_path_unchanged=($beforePath -eq [Environment]::GetEnvironmentVariable('PATH','Machine'))
 if(!$report.machine_path_unchanged){throw 'Machine PATH changed'}
 $report.status='pass'
}catch{$report.status='fail';$report.error=$_.Exception.Message}
finally{
 if($guard){$guard.Dispose()}
 if($process -and !$process.HasExited){& "$env:SystemRoot/System32/taskkill.exe" /PID $process.Id /T /F|Out-Null}
 if($credentialCreated){$null=[PapoInstallerCredential]::Delete($credential,1,0)}
 foreach($marker in $markers){Remove-Item $marker -Force -ErrorAction SilentlyContinue}
 # EN-US: Failure artifacts keep the installed program for inspection; cleanup only touches our fixture data.
 # PT-BR: Em caso de falha, o programa é preservado para inspeção; a limpeza só remove dados da fixture.
 $report.finished_utc=[DateTime]::UtcNow.ToString('o')
 $report|ConvertTo-Json -Depth 10|Set-Content (Join-Path $OutputDirectory 'installer-runtime.json') -Encoding UTF8
}
if($report.status -ne 'pass'){throw "Installer acceptance failed: $($report.error)"}
$global:LASTEXITCODE=0
