# EN-US: Exercise only shipped files from an unrelated working directory.
# PT-BR: Testa apenas arquivos distribuídos a partir de outro diretório de trabalho.
$ErrorActionPreference = 'Stop'
$archive = Get-ChildItem "$PSScriptRoot/../../download/Papo-*-windows-x86_64-dev.zip" | Select-Object -First 1
if (!$archive) { throw 'Portable archive missing' }
$destination = Join-Path $env:RUNNER_TEMP 'Papo portable á é ç'
Expand-Archive $archive.FullName $destination
$package = (Get-ChildItem $destination -Directory | Select-Object -First 1).FullName
$launcher = Join-Path $package 'Papo.exe'
# EN-US: Clear inherited developer discovery paths without changing machine settings.
# PT-BR: Limpa caminhos herdados de desenvolvimento sem alterar ajustes da máquina.
Get-ChildItem Env: | Where-Object Name -Match '^(PAPO_|PYTHON|GST_|GI_|GTK_PATH|GIO_|GDK_PIXBUF_|GSETTINGS_SCHEMA_DIR|FONTCONFIG_|XDG_)' | ForEach-Object { Remove-Item "Env:$($_.Name)" }
$env:PATH = "$env:SystemRoot\System32;$env:SystemRoot;$env:SystemRoot\System32\WindowsPowerShell\v1.0"
$env:GSK_RENDERER = 'cairo'
$env:GTK_A11Y = 'none'
$env:PYTHONUTF8 = '1'
$beforePath = [Environment]::GetEnvironmentVariable('PATH', 'Machine')
$unrelated = New-Item -ItemType Directory -Force (Join-Path $env:RUNNER_TEMP 'unrelated cwd')
Set-Location $unrelated.FullName
function Invoke-Check([string]$option) {
    & $launcher $option
    if ($LASTEXITCODE -ne 0) { throw "Portable check failed: $option ($LASTEXITCODE)" }
}
Invoke-Check '--version'
Invoke-Check '--platform-info'
Invoke-Check '--check-packaged-runtime'
# EN-US: Relocation must invalidate absolute plugin paths in the per-user registry.
# PT-BR: A mudança deve invalidar caminhos absolutos de plugins no registro do usuário.
$relocated = "$package relocated"
Move-Item $package $relocated
$package = $relocated
$launcher = Join-Path $package 'Papo.exe'
Invoke-Check '--check-packaged-runtime'
$python = Join-Path $package 'runtime/bin/python.exe'
Move-Item $python "$python.disabled"
try {
    & $launcher --check-packaged-runtime
    if ($LASTEXITCODE -eq 0) { throw 'Missing bundled Python incorrectly succeeded' }
} finally { Move-Item "$python.disabled" $python }
$manifest = Get-Content (Join-Path $package 'runtime/RUNTIME-MANIFEST.json') -Raw | ConvertFrom-Json
foreach ($file in $manifest.files) {
    $actual = Get-FileHash (Join-Path $package $file.path) -Algorithm SHA256
    if ($actual.Hash -ne $file.sha256) { throw "Package modified during checks: $($file.path)" }
}
if (Get-ChildItem $package -Filter '*.pyc' -Recurse) { throw 'Bytecode written inside package' }
if (Get-ChildItem $package -Filter 'registry*.bin' -Recurse) { throw 'Plugin registry written inside package' }
if ($beforePath -ne [Environment]::GetEnvironmentVariable('PATH', 'Machine')) { throw 'Machine PATH changed' }
Write-Output 'Portable runtime, relocation, immutable files and missing-Python checks passed.'
