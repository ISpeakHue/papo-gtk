# EN-US: Explicit integration test for a disposable standard-user Windows desktop.
# PT-BR: Teste de integração explícito para desktop Windows descartável com usuário padrão.
param([Parameter(Mandatory=$true)][string]$Archive,[Parameter(Mandatory=$true)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$OutputDirectory=(Get-Item $OutputDirectory).FullName
$report=[ordered]@{schema=1;status='running';started_utc=[DateTime]::UtcNow.ToString('o')}
$registered=$false;$ownedWindow=$null;$bootstrap=$null;$counter=0
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class PapoIdentityNative {
 [DllImport("shcore.dll")] public static extern int GetProcessDpiAwareness(IntPtr process,out int awareness);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd,int command);
 [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint x,uint y,uint data,UIntPtr extra);
 [DllImport("user32.dll")] public static extern void keybd_event(byte key,byte scan,uint flags,UIntPtr extra);
 [DllImport("shell32.dll",CharSet=CharSet.Unicode)] public static extern uint ExtractIconEx(string file,int index,out IntPtr large,out IntPtr small,uint count);
 [DllImport("user32.dll")] public static extern bool DestroyIcon(IntPtr icon);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr LoadLibraryEx(string file,IntPtr handle,uint flags);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr FindResource(IntPtr module,IntPtr name,IntPtr type);
 [DllImport("kernel32.dll")] public static extern uint SizeofResource(IntPtr module,IntPtr resource);
 [DllImport("kernel32.dll")] public static extern IntPtr LoadResource(IntPtr module,IntPtr resource);
 [DllImport("kernel32.dll")] public static extern IntPtr LockResource(IntPtr resource);
 [DllImport("kernel32.dll")] public static extern bool FreeLibrary(IntPtr module);
}
'@
function Screenshot([string]$Name){
 $screen=[Windows.Forms.SystemInformation]::VirtualScreen;$bitmap=New-Object Drawing.Bitmap($screen.Width,$screen.Height);$graphics=[Drawing.Graphics]::FromImage($bitmap)
 try{$graphics.CopyFromScreen($screen.Left,$screen.Top,0,0,$bitmap.Size);$bitmap.Save((Join-Path $OutputDirectory $Name),[Drawing.Imaging.ImageFormat]::Png)}finally{$graphics.Dispose();$bitmap.Dispose()}
}
function Cli([string[]]$Arguments){
 $script:counter++;$prefix=Join-Path $OutputDirectory ('cli-'+$script:counter)
 $p=Start-Process $script:cli -ArgumentList ($Arguments -join ' ') -NoNewWindow -PassThru -RedirectStandardOutput ($prefix+'.stdout.txt') -RedirectStandardError ($prefix+'.stderr.txt')
 $null=$p.Handle;if(!$p.WaitForExit(120000)){$p.Kill();throw 'Identity CLI timed out'};$p.WaitForExit()
 if($p.ExitCode -ne 0){throw ('Identity CLI failed: '+($Arguments -join ' ')+'; '+(Get-Content ($prefix+'.stderr.txt') -Raw))}
 return (Get-Content ($prefix+'.stdout.txt') -Raw -Encoding UTF8)
}
function Status { return (Cli @('--startup-info') | ConvertFrom-Json) }
function WaitWindow {
 for($i=0;$i -lt 120;$i++){
  $p=Get-Process -Name papo-gtk -ErrorAction SilentlyContinue | Where-Object {$_.MainWindowHandle -ne 0 -and $_.Path -eq (Join-Path $script:package 'runtime\bin\papo-gtk.exe')} | Select-Object -First 1
  if($p){return $p};Start-Sleep -Milliseconds 500
 };throw 'Papo window did not appear'
}
function CloseWindow {
 if($script:ownedWindow -and !$script:ownedWindow.HasExited){if(!$script:ownedWindow.CloseMainWindow()){throw 'Cannot close Papo window'};if(!$script:ownedWindow.WaitForExit(15000)){throw 'Papo did not stop'}}
 if($script:bootstrap -and !$script:bootstrap.WaitForExit(15000)){throw 'GUI bootstrap did not stop'}
}
function WaitDelivery([long]$Previous){
 for($i=0;$i -lt 40;$i++){$state=Status;if($state.delivered -gt $Previous){return $state};Start-Sleep -Milliseconds 200};throw 'Activation did not reach GTK'
}
function ClickToast {
 $text='Teste de identidade e ativação. Clique para abrir o Papo.'
 $condition=New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,$text)
 for($i=0;$i -lt 10;$i++){
  $element=[Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)
  if($element){$rect=$element.Current.BoundingRectangle;if($rect.Width -gt 0 -and $rect.Height -gt 0){Screenshot 'toast-before-click.png';[PapoIdentityNative]::SetCursorPos([int]($rect.Left+$rect.Width/2),[int]($rect.Top+$rect.Height/2)) | Out-Null;[PapoIdentityNative]::mouse_event(2,0,0,0,[UIntPtr]::Zero);[PapoIdentityNative]::mouse_event(4,0,0,0,[UIntPtr]::Zero);return}}
  if($i -eq 3){[PapoIdentityNative]::keybd_event(0x5b,0,0,[UIntPtr]::Zero);[PapoIdentityNative]::keybd_event(0x4e,0,0,[UIntPtr]::Zero);[PapoIdentityNative]::keybd_event(0x4e,0,2,[UIntPtr]::Zero);[PapoIdentityNative]::keybd_event(0x5b,0,2,[UIntPtr]::Zero)}
  Start-Sleep -Seconds 1
 };Screenshot 'toast-not-found.png';throw 'Test toast was not available for a real UI click'
}
try{
 $identity=[Security.Principal.WindowsIdentity]::GetCurrent();$principal=New-Object Security.Principal.WindowsPrincipal($identity)
 if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator) -or @($identity.Groups|ForEach-Object Value) -contains 'S-1-5-32-544'){throw 'Use a standard fixture account'}
 if(Get-Process -Name papo-gtk -ErrorAction SilentlyContinue){throw 'Close other Papo instances before explicit identity testing'}
 $report.user=@{name=$identity.Name;administrator=$false}
 $version=Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion';$os=Get-CimInstance Win32_OperatingSystem
 $report.os=@{caption=$os.Caption;version=$os.Version;display_version=$version.DisplayVersion;build=$version.CurrentBuildNumber;ubr=$version.UBR;edition=$version.EditionID}
 $machinePath=[Environment]::GetEnvironmentVariable('PATH','Machine')
 $archiveFile=Get-Item $Archive;$hash=(Get-FileHash $archiveFile.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
 if($hash -ne (Get-Content ($archiveFile.FullName+'.sha256') -Raw).Split(' ')[0]){throw 'ZIP checksum mismatch'}
 $report.archive=@{name=$archiveFile.Name;sha256=$hash}
 $work=Join-Path $env:USERPROFILE ('WI'+[guid]::NewGuid().ToString('N').Substring(0,6));$accents=[string][char]0xe1+[char]0xe9+[char]0xe7
 $destination=Join-Path $work ('Papo '+$accents);Add-Type -AssemblyName System.IO.Compression.FileSystem;[IO.Compression.ZipFile]::ExtractToDirectory($archiveFile.FullName,$destination)
 $script:package=(Get-ChildItem $destination -Directory|Select-Object -First 1).FullName;$script:cli=Join-Path $package 'Papo.com';$gui=Join-Path $package 'Papo.exe'
 $build=Get-Content (Join-Path $package 'BUILD-INFO.json') -Raw -Encoding UTF8|ConvertFrom-Json;$report.source_commit=$build.source.commit;$report.source_dirty=$build.source_dirty;$report.runtime_manifest_sha256=$build.runtime.manifest_sha256
 if($build.source_dirty){throw 'Expected clean-source package'}
 $report.resources=@()
 foreach($relative in @('Papo.exe','Papo.com','runtime\bin\papo-gtk.exe','runtime\bin\papo-shell.exe')){
  $file=Join-Path $package $relative;$data=[IO.File]::ReadAllBytes($file);$pe=[BitConverter]::ToUInt32($data,0x3c);$subsystem=[BitConverter]::ToUInt16($data,$pe+24+68)
  $expectedSubsystem=if($relative -in @('Papo.exe','runtime\bin\papo-gtk.exe')){2}else{3};if($subsystem -ne $expectedSubsystem){throw "Incorrect subsystem: $relative"}
  $info=[Diagnostics.FileVersionInfo]::GetVersionInfo($file);if($info.ProductName -ne 'Papo' -or $info.FileVersion -ne ($build.version+'.0')){throw "Incorrect version identity: $relative"}
  $large=[IntPtr]::Zero;$small=[IntPtr]::Zero;if([PapoIdentityNative]::ExtractIconEx($file,0,[ref]$large,[ref]$small,1) -ne 1){throw "Missing native icon: $relative"};[PapoIdentityNative]::DestroyIcon($large)|Out-Null;[PapoIdentityNative]::DestroyIcon($small)|Out-Null
  $module=[PapoIdentityNative]::LoadLibraryEx($file,[IntPtr]::Zero,2)
  try{$resource=[PapoIdentityNative]::FindResource($module,[IntPtr]1,[IntPtr]24);$size=[PapoIdentityNative]::SizeofResource($module,$resource);$pointer=[PapoIdentityNative]::LockResource([PapoIdentityNative]::LoadResource($module,$resource));$bytes=New-Object byte[] $size;[Runtime.InteropServices.Marshal]::Copy($pointer,$bytes,0,$size);$manifest=[Text.Encoding]::UTF8.GetString($bytes);if($manifest -notmatch 'asInvoker' -or $manifest -notmatch '>system</dpiAwareness>'){throw "Incorrect manifest: $relative"}}finally{[PapoIdentityNative]::FreeLibrary($module)|Out-Null}
  $report.resources+=@{file=$relative;subsystem=$subsystem;version=$info.FileVersion;icon=$true;manifest=$true}
 }
 $env:GSK_RENDERER='cairo';$env:GTK_A11Y='none';$env:PYTHONUTF8='1'
 $report.runtime=Cli @('--check-packaged-runtime')
 $report.activation_policy=Cli @('--check-activation-policy')
 Cli @('--register-desktop')|Out-Null;$registered=$true;$report.desktop=Cli @('--desktop-info')|ConvertFrom-Json
 $shortcut=Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Papo.lnk'
 # EN-US: Normal launch uses the GUI entry point, from an unrelated working directory.
 # PT-BR: A inicialização normal usa a entrada gráfica a partir de outro diretório.
 $script:bootstrap=Start-Process $gui -WorkingDirectory $env:SystemRoot -PassThru -RedirectStandardOutput (Join-Path $OutputDirectory 'application.stdout.txt') -RedirectStandardError (Join-Path $OutputDirectory 'application.stderr.txt');$null=$bootstrap.Handle
 $script:ownedWindow=WaitWindow;[PapoIdentityNative]::ShowWindow($ownedWindow.MainWindowHandle,3)|Out-Null;Start-Sleep -Seconds 2;$awareness=0;if([PapoIdentityNative]::GetProcessDpiAwareness($ownedWindow.Handle,[ref]$awareness) -ne 0 -or $awareness -ne 1){throw 'GTK DPI mode does not match its manifest'};$report.dpi=@{awareness='system';window_dpi=[PapoIdentityNative]::GetDpiForWindow($ownedWindow.MainWindowHandle)};$normal=Status;if($normal.console){throw 'Normal GTK process owns a console'};Screenshot 'normal-window.png'
 Start-Process $shortcut; $shortcutState=WaitDelivery $normal.delivered;if($shortcutState.pid -ne $normal.pid){throw 'Shortcut started a duplicate instance'};$report.shortcut_activation=$shortcutState
 [PapoIdentityNative]::ShowWindow($ownedWindow.MainWindowHandle,6)|Out-Null
 Start-Process ('papo-notification://open/'+[guid]::NewGuid().ToString());$warm=WaitDelivery $shortcutState.delivered
 if($warm.pid -ne $normal.pid -or [PapoIdentityNative]::IsIconic($ownedWindow.MainWindowHandle)){throw 'Protocol did not restore the owning window'};$report.protocol_warm=$warm
 [PapoIdentityNative]::ShowWindow($ownedWindow.MainWindowHandle,6)|Out-Null
 Cli @('--test-toast',[guid]::NewGuid().ToString())|Out-Null;Start-Sleep -Seconds 2;$report.toast_history=Cli @('--test-toast-info')|ConvertFrom-Json;if(!$report.toast_history.test_present){throw 'Toast not retained under the registered identity'}
 ClickToast;$warmToast=WaitDelivery $warm.delivered;if($warmToast.pid -ne $normal.pid){throw 'Warm toast opened another instance'};$report.toast_warm=$warmToast;Screenshot 'warm-toast-activated.png'
 CloseWindow;$ownedWindow=$null;$bootstrap=$null
 Cli @('--test-toast',[guid]::NewGuid().ToString())|Out-Null;Start-Sleep -Seconds 2;ClickToast;$script:ownedWindow=WaitWindow;Start-Sleep -Seconds 1;$cold=Status
 if($cold.pid -eq $normal.pid -or $cold.console -or $cold.delivered -lt 1){throw 'Cold toast activation failed'};$report.toast_cold=$cold;Screenshot 'cold-toast-activated.png'
 # EN-US: Retain native shell visuals for inspection; do not infer DPI acceptance from them.
 # PT-BR: Guarda imagens do shell nativo para inspeção; não deduz validação de DPI a partir delas.
 Start-Process explorer.exe -ArgumentList ('/select,"'+$gui+'"');Start-Sleep -Seconds 3;Screenshot 'explorer.png'
 [Windows.Forms.SendKeys]::SendWait('^{ESC}');Start-Sleep -Seconds 1;[Windows.Forms.SendKeys]::SendWait('Papo');Start-Sleep -Seconds 3;Screenshot 'start-menu.png';[Windows.Forms.SendKeys]::SendWait('{ESC}')
 CloseWindow;$ownedWindow=$null;$bootstrap=$null
 if($machinePath -ne [Environment]::GetEnvironmentVariable('PATH','Machine')){throw 'Machine PATH changed'};$report.machine_path_unchanged=$true;$report.status='pass'
}catch{$report.status='fail';$report.error=$_.Exception.Message;$_|Out-String|Set-Content (Join-Path $OutputDirectory 'failure.txt') -Encoding UTF8}
finally{
 if($ownedWindow -and !$ownedWindow.HasExited){$ownedWindow.CloseMainWindow()|Out-Null;if(!$ownedWindow.WaitForExit(10000)){$ownedWindow.Kill()}}
 if($bootstrap -and !$bootstrap.HasExited){$bootstrap.Kill()}
 if($registered){try{Cli @('--clear-test-toast')|Out-Null;Cli @('--unregister-desktop')|Out-Null;$report.registration_removed=$true}catch{$report.status='fail';$report.cleanup_error=$_.Exception.Message}}
 $report.finished_utc=[DateTime]::UtcNow.ToString('o');$report|ConvertTo-Json -Depth 12|Set-Content (Join-Path $OutputDirectory 'identity.json') -Encoding UTF8
}
if($report.status -ne 'pass'){exit 1}
