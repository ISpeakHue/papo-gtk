# EN-US: Run the shared developer commands inside one explicit UCRT64 installation.
# PT-BR: Executa os comandos de desenvolvimento em uma instalação UCRT64 explícita.
[CmdletBinding()]
param(
    [ValidateSet('Check', 'Setup', 'Build', 'Test', 'Run', 'Debug')]
    [string]$Action = 'Check',
    [string]$MsysRoot = (Join-Path $env:LOCALAPPDATA 'papo-dev/msys64'),
    [switch]$ExcludeHip
)
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Use scripts/windows-dev.sh from an MSYS2 UCRT64 terminal.' }
$bash = Join-Path $MsysRoot 'usr/bin/bash.exe'
if (!(Test-Path -LiteralPath $bash -PathType Leaf)) {
    throw "MSYS2 not found at $MsysRoot. Install/extract official MSYS2 there, or pass -MsysRoot C:\msys64. See MULTIPLATFORM.md."
}
$script = Join-Path $PSScriptRoot 'windows-dev.sh'
$saved = @{}
foreach ($name in @('MSYSTEM', 'CHERE_INVOKING', 'MSYS2_PATH_TYPE', 'PAPO_DEV_EXCLUDE_HIP')) {
    $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
try {
    $env:MSYSTEM = 'UCRT64'
    $env:CHERE_INVOKING = '1'
    $env:MSYS2_PATH_TYPE = 'strict'
    [Environment]::SetEnvironmentVariable('PAPO_DEV_EXCLUDE_HIP', $(if ($ExcludeHip) { '1' } else { $null }), 'Process')
    # EN-US: Pass paths as positional arguments, never interpolate them into shell code.
    # PT-BR: Passa caminhos como argumentos posicionais, sem interpolá-los no código do shell.
    & $bash --login ($script.Replace('\', '/')) $Action.ToLowerInvariant()
    $result = $LASTEXITCODE
} finally {
    foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process') }
}
if ($result -ne 0) { throw "Windows developer command '$Action' failed (exit $result)." }
