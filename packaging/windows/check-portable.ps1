# EN-US: Execute shipped files without developer discovery paths; retain bounded, hash-bound evidence.
# PT-BR: Executa arquivos distribuídos sem caminhos de desenvolvimento; registra evidências limitadas e vinculadas por hashes.
param(
    [string]$Archive,
    [string]$WorkDirectory = $(if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [IO.Path]::GetTempPath() }),
    [string]$OutputDirectory = 'target/windows-portable'
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$OutputDirectory = (Get-Item $OutputDirectory).FullName
$report = [ordered]@{schema=1;status='running';cases=@();started_utc=[DateTime]::UtcNow.ToString('o')}
$originalLocation = Get-Location
$destination = $null
$process = $null
$environment = @{}
Get-ChildItem Env: | ForEach-Object { $environment[$_.Name] = $_.Value }
function Invoke-Check([string]$name, [string]$option, [bool]$expectFailure=$false) {
    $stdout = Join-Path $OutputDirectory ($name+'.stdout.txt')
    $stderr = Join-Path $OutputDirectory ($name+'.stderr.txt')
    $script:process = Start-Process $launcher -ArgumentList $option -WorkingDirectory $unrelated -NoNewWindow -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $null = $process.Handle
    $timedOut = !$process.WaitForExit(360000)
    if ($timedOut) { & "$env:SystemRoot/System32/taskkill.exe" /PID $process.Id /T /F | Out-Null }
    $process.WaitForExit()
    $pass = !$timedOut -and (($expectFailure -and $process.ExitCode -ne 0) -or (!$expectFailure -and $process.ExitCode -eq 0))
    $report.cases += @{name=$name;status=$(if($pass){'pass'}else{'fail'});exit_code=$process.ExitCode;timed_out=$timedOut;expected_failure=$expectFailure}
    if (!$pass) { throw "Portable check failed: $name; inspect retained stdout/stderr" }
    return [IO.File]::ReadAllText($stdout, [Text.Encoding]::UTF8)
}
function Assert-Missing([string]$name, [string]$path, [string]$option) {
    if (!(Test-Path -LiteralPath $path)) { throw "Required bundled component missing before test: $name" }
    Move-Item -LiteralPath $path -Destination ($path+'.disabled')
    try { $null = Invoke-Check $name $option $true }
    finally { Move-Item -LiteralPath ($path+'.disabled') -Destination $path }
}
try {
    $zip = if ($Archive) { Get-Item $Archive } else { Get-ChildItem "$PSScriptRoot/../../download/Papo-*-windows-x86_64-dev.zip" | Select-Object -First 1 }
    if (!$zip) { throw 'Portable archive missing' }
    $hash = (Get-FileHash $zip.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    $expected = (Get-Content ($zip.FullName+'.sha256') -Raw -Encoding UTF8).Split(' ')[0].Trim()
    if ($expected -notmatch '^[0-9a-fA-F]{64}$' -or $hash -ne $expected) { throw 'Portable ZIP checksum mismatch' }
    $report.archive = @{name=$zip.Name;sha256=$hash}
    $accents = [string][char]0xe1+' '+[char]0xe9+' '+[char]0xe7
    $destination = Join-Path $WorkDirectory ('Papo portable '+$accents+' '+[guid]::NewGuid().ToString('N'))
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [IO.Compression.ZipFile]::ExtractToDirectory($zip.FullName, $destination)
    $package = (Get-ChildItem $destination -Directory | Select-Object -First 1).FullName
    $buildPath = Join-Path $package 'BUILD-INFO.json'
    $build = Get-Content $buildPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($build.source_dirty -or $build.source.status) { throw 'Dirty package source' }
    $report.source_commit = $build.source.commit
    $report.build_info_sha256 = (Get-FileHash $buildPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $report.runtime_manifest_sha256 = $build.runtime.manifest_sha256
    $report.os = @{version=[Environment]::OSVersion.VersionString;ci_image=$env:ImageOS}
    # EN-US: Restore the caller environment in finally; never alter machine-wide paths or settings.
    # PT-BR: Restaura o ambiente do chamador no finally; nunca altera caminhos ou ajustes da máquina.
    Get-ChildItem Env: | Where-Object Name -Match '^(PAPO_|PYTHON|GST_|GI_|GTK_|GSK_|GDK_|GIO_|GSETTINGS_SCHEMA_DIR|FONTCONFIG_|XDG_)' | ForEach-Object { Remove-Item "Env:$($_.Name)" }
    $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot;$env:SystemRoot\System32\WindowsPowerShell\v1.0"
    $env:GSK_RENDERER='cairo'; $env:GTK_A11Y='none'; $env:PYTHONUTF8='1'
    $beforePath = [Environment]::GetEnvironmentVariable('PATH', 'Machine')
    $unrelated = Join-Path $destination 'unrelated cwd'
    New-Item -ItemType Directory $unrelated | Out-Null
    Set-Location $unrelated
    $launcher = Join-Path $package 'Papo.com'
    $null = Invoke-Check 'version' '--version'
    $caps = Invoke-Check 'capabilities' '--platform-info' | ConvertFrom-Json
    if ($caps.os -ne 'windows' -or $caps.credential_store -ne 'windows_credential_store' -or !$caps.desktop_notifications -or !$caps.isolated_media_helpers -or $caps.screen_capture) { throw 'Unexpected Windows capabilities; capture must remain opt-in preview' }
    $report.capabilities = $caps
    $null = Invoke-Check 'runtime' '--check-packaged-runtime'
    $notifications = Invoke-Check 'notifications' '--check-notifications' | ConvertFrom-Json
    if ($notifications.status -ne 'pass' -or !$notifications.encrypted_storage -or !$notifications.cold_activation -or !$notifications.winrt_toast -or $notifications.desktop_delivery) { throw 'Notification runtime probe incomplete' }
    $storage = Invoke-Check 'storage' '--check-windows-storage' | ConvertFrom-Json
    if ($storage.status -ne 'pass' -or $storage.credentials.status -ne 'pass' -or $storage.files.status -ne 'pass' -or $storage.desktop.status -ne 'pass' -or !$storage.scratch_removed) { throw 'Disposable native storage probe incomplete' }
    $isolationText = Invoke-Check 'isolation' '--check-media-runtime'
    $media = ($isolationText.Split("`n") | Where-Object { $_.Trim().StartsWith('{') } | Select-Object -Last 1) | ConvertFrom-Json
    if ($media.status -ne 'pass' -or $media.posters -ne 3 -or $media.animations -ne 3 -or $media.isolation.backend -ne 'appcontainer_job' -or $media.isolation.tcp4_tcp6_udp_requests -ne 0) { throw 'Isolated conversion probe incomplete' }
    foreach ($field in @('private_files_denied','input_runtime_writes_denied','credential_read_denied','handle_inheritance_denied','stdout_stderr_limits','committed_memory_limit','internal_deadline','cancellation_tree_cleanup','parent_exit_tree_cleanup','orphan_reaping','cancellation_slot_cleanup')) {
        if ($media.isolation.$field -ne $true) { throw "Missing isolation assertion: $field" }
    }
    $report.isolation = $media
    # EN-US: Relocation must invalidate absolute plugin paths in per-user caches.
    # PT-BR: A mudança deve invalidar caminhos absolutos dos plugins nos caches do usuário.
    $relocated = "$package relocated"
    Move-Item -LiteralPath $package -Destination $relocated
    $package = $relocated; $launcher = Join-Path $package 'Papo.com'
    $null = Invoke-Check 'relocated-runtime' '--check-packaged-runtime'
    $null = Invoke-Check 'relocated-notifications' '--check-notifications'
    Assert-Missing 'missing-python' (Join-Path $package 'runtime/bin/python.exe') '--check-packaged-runtime'
    $gtk = Get-ChildItem (Join-Path $package 'runtime/bin') -Filter 'libgtk-4-*.dll' | Select-Object -First 1
    if (!$gtk) { throw 'Bundled GTK DLL missing' }
    Assert-Missing 'missing-gtk' $gtk.FullName '--version'
    Assert-Missing 'missing-capture' (Join-Path $package 'runtime/bin/papo-capture.dll') '--check-packaged-runtime'
    Assert-Missing 'missing-shell' (Join-Path $package 'runtime/bin/papo-shell.exe') '--check-packaged-runtime'
    $manifest = Get-Content (Join-Path $package 'runtime/RUNTIME-MANIFEST.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    foreach ($file in $manifest.files) {
        if ((Get-FileHash (Join-Path $package $file.path) -Algorithm SHA256).Hash -ne $file.sha256) { throw "Package modified: $($file.path)" }
    }
    if ((Get-ChildItem $package -Filter '*.pyc' -Recurse) -or (Get-ChildItem $package -Filter 'registry*.bin' -Recurse)) { throw 'Caches written inside package' }
    $report.immutable_runtime = $true
    $report.machine_path_unchanged = ($beforePath -eq [Environment]::GetEnvironmentVariable('PATH', 'Machine'))
    if (!$report.machine_path_unchanged) { throw 'Machine PATH changed' }
    $report.status = 'pass'
} catch { $report.status='fail'; $report.error=$_.Exception.Message }
finally {
    if ($process -and !$process.HasExited) { & "$env:SystemRoot/System32/taskkill.exe" /PID $process.Id /T /F | Out-Null }
    Set-Location $originalLocation
    if ($destination) {
        try { Remove-Item -LiteralPath $destination -Recurse -Force; $report.extracted_package_removed=$true }
        catch { $report.status='fail'; $report.cleanup_error=$_.Exception.Message }
    }
    Get-ChildItem Env: | Where-Object { !$environment.ContainsKey($_.Name) } | ForEach-Object { Remove-Item "Env:$($_.Name)" }
    foreach ($name in $environment.Keys) { Set-Item "Env:$name" $environment[$name] }
    $report.finished_utc=[DateTime]::UtcNow.ToString('o')
    $report | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $OutputDirectory 'portable-runtime.json') -Encoding UTF8
}
if ($report.status -ne 'pass') { throw "Portable runtime gate failed: $($report.error) $($report.cleanup_error)" }
$global:LASTEXITCODE=0
