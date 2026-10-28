# EN-US: Run inside a disposable offline Windows guest, as a standard local user.
# PT-BR: Executa em um Windows descartável e offline, com usuário local padrão.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$OutputEncoding = [Console]::OutputEncoding
$media = $PSScriptRoot
$output = $null
foreach ($drive in [IO.DriveInfo]::GetDrives()) {
    if ($drive.IsReady -and (Test-Path (Join-Path $drive.RootDirectory.FullName 'PAPO_RESULTS.marker'))) {
        $output = Join-Path $drive.RootDirectory.FullName 'results'
        break
    }
}
if (!$output) { throw 'Disposable results volume not found' }
New-Item -ItemType Directory -Force $output | Out-Null
$report = [ordered]@{ schema = 1; status = 'running'; started_utc = [DateTime]::UtcNow.ToString('o') }
$started = $null
try {
    Remove-Item "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\Startup\PapoAcceptance.cmd" -ErrorAction SilentlyContinue
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    $admin = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    $groups = @($identity.Groups | ForEach-Object Value)
    if ($admin -or $groups -contains 'S-1-5-32-544') { throw 'Acceptance must use a standard account outside Administrators' }
    $report.user = @{ name = $identity.Name; profile = $env:USERPROFILE; administrator = $admin; groups = $groups }
    $version = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
    $os = Get-CimInstance Win32_OperatingSystem
    $report.os = @{ caption = $os.Caption; version = $os.Version; architecture = $os.OSArchitecture; display_version = $version.DisplayVersion; build = $version.CurrentBuildNumber; ubr = $version.UBR; edition = $version.EditionID; build_lab = $version.BuildLabEx }
    $report.hotfixes = @(Get-HotFix | Select-Object HotFixID, Description)
    $report.network_adapters = @(Get-NetAdapter -Physical | Select-Object Name, InterfaceDescription, Status)
    if ($report.network_adapters.Count) { throw 'Clean VM must have no physical or virtual network interface' }
    $machinePath = [Environment]::GetEnvironmentVariable('PATH', 'Machine')
    $report.machine_path_before = $machinePath
    $tools = @()
    foreach ($name in @('python.exe', 'python3.exe', 'py.exe', 'rustc.exe', 'cargo.exe', 'pacman.exe', 'gst-launch-1.0.exe', 'gcc.exe', 'gtk4-launch.exe')) {
        foreach ($command in @(Get-Command $name -CommandType Application -ErrorAction SilentlyContinue)) {
            $item = Get-Item $command.Source -Force
            $alias = $item.FullName -like '*\Microsoft\WindowsApps\*' -and $item.Length -eq 0
            $tools += @{ name = $name; path = $item.FullName; app_execution_alias = $alias }
            if (!$alias) { throw "External development/runtime executable found: $name" }
        }
    }
    $report.external_commands = $tools
    $apps = @('HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*', 'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*', 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*') |
        ForEach-Object { Get-ItemProperty $_ -ErrorAction SilentlyContinue } |
        Where-Object DisplayName | Select-Object DisplayName, DisplayVersion
    $report.installed_applications = @($apps)
    if ($apps | Where-Object DisplayName -Match '^(Python|MSYS2|Rust|GStreamer|GTK|libadwaita)') { throw 'External runtime is installed' }
    foreach ($path in @('C:\msys64', 'C:\msys32', 'C:\gstreamer', 'C:\Program Files\GStreamer', "$env:LOCALAPPDATA\Programs\Python", "$env:USERPROFILE\.cargo\bin\rustc.exe")) {
        if (Test-Path $path) { throw "External runtime directory found: $path" }
    }
    $archive = Get-ChildItem $media -Filter 'Papo-*-windows-x86_64-dev.zip' | Select-Object -First 1
    if (!$archive) { throw 'Acceptance archive missing' }
    $report.archive = @{ name = $archive.Name; sha256 = (Get-FileHash $archive.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
    $work = Join-Path $env:USERPROFILE ('PV' + [guid]::NewGuid().ToString('N').Substring(0, 6))
    New-Item -ItemType Directory -Force $work | Out-Null
    $script = Join-Path $media 'check-portable.ps1'
    $arguments = '-NoProfile -ExecutionPolicy Bypass -File "' + $script + '" -Archive "' + $archive.FullName + '" -WorkDirectory "' + $work + '"'
    $test = Start-Process -FilePath "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -ArgumentList $arguments -RedirectStandardOutput (Join-Path $output 'runtime.stdout.txt') -RedirectStandardError (Join-Path $output 'runtime.stderr.txt') -PassThru
    # EN-US: Retain the native handle so Windows PowerShell 5.1 preserves the exit code.
    # PT-BR: Retém o handle nativo para o Windows PowerShell 5.1 preservar o código de saída.
    $null = $test.Handle
    if (!$test.WaitForExit(1200000)) { $test.Kill(); throw 'Portable acceptance timed out' }
    if ($test.ExitCode -ne 0) { throw "Portable acceptance failed: $($test.ExitCode)" }
    $transcript = Get-Content (Join-Path $output 'runtime.stdout.txt') -Raw -Encoding UTF8
    if ($transcript -notmatch 'missing-component checks passed') { throw 'Portable acceptance success marker missing' }
    $report.runtime_checks = 'pass'
    $info = Get-ChildItem $work -Recurse -Filter BUILD-INFO.json | Select-Object -First 1
    $package = $info.Directory.FullName
    $build = Get-Content $info.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
    $report.source_commit = $build.source.commit
    $report.source_dirty = $build.source_dirty
    $report.runtime_manifest_sha256 = $build.runtime.manifest_sha256
    if ($build.source_dirty) { throw 'Artifact source was dirty' }
    $report.package = $package
    $env:GSK_RENDERER = 'cairo'
    $env:GTK_A11Y = 'none'
    $started = Start-Process -FilePath (Join-Path $package 'Papo.exe') -WorkingDirectory $env:SystemRoot -RedirectStandardOutput (Join-Path $output 'application.stdout.txt') -RedirectStandardError (Join-Path $output 'application.stderr.txt') -PassThru
    $null = $started.Handle
    $window = $null
    for ($i = 0; $i -lt 120; $i++) {
        Start-Sleep -Milliseconds 500
        $window = Get-Process -Name papo-gtk -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
        if ($window) { break }
        if ($started.HasExited) { throw "Normal app startup exited: $($started.ExitCode)" }
    }
    if (!$window) { throw 'Normal application window did not appear' }
    Start-Sleep -Seconds 3
    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -AssemblyName System.Drawing
    $screen = [Windows.Forms.SystemInformation]::VirtualScreen
    $bitmap = New-Object Drawing.Bitmap($screen.Width, $screen.Height)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($screen.Left, $screen.Top, 0, 0, $bitmap.Size)
        $bitmap.Save((Join-Path $output 'application.png'), [Drawing.Imaging.ImageFormat]::Png)
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
    $report.normal_startup = @{ status = 'pass'; window_title = $window.MainWindowTitle; screenshot = 'application.png'; renderer = 'cairo' }
    if (!$window.CloseMainWindow()) { throw 'Cannot close the normal application window' }
    if (!$window.WaitForExit(15000)) { $window.Kill(); throw 'Normal application did not exit after closing its window' }
    if (!$started.WaitForExit(15000)) { $started.Kill(); throw 'Bootstrap remained running after the application exited' }
    if ($started.ExitCode -ne 0) { throw "Normal application failed on exit: $($started.ExitCode)" }
    if ($machinePath -ne [Environment]::GetEnvironmentVariable('PATH', 'Machine')) { throw 'Machine PATH changed' }
    $report.machine_path_unchanged = $true
    $report.status = 'pass'
} catch {
    $report.status = 'fail'
    $report.error = $_.Exception.Message
    $_ | Out-String | Set-Content (Join-Path $output 'failure.txt') -Encoding UTF8
} finally {
    if ($started -and !$started.HasExited) { $started.Kill() }
    $report.finished_utc = [DateTime]::UtcNow.ToString('o')
    $report | ConvertTo-Json -Depth 12 | Set-Content (Join-Path $output 'acceptance.json') -Encoding UTF8
    # EN-US: Close the VM so the host reads the results disk only after Windows flushes it.
    # PT-BR: Encerra a VM para o host ler o disco de resultados após o Windows gravá-lo.
    Start-Sleep -Seconds 3
    & "$env:SystemRoot\System32\shutdown.exe" /s /t 0
}
