# EN-US: Disposable packaged desktop checks; native key injection requires an explicit flag.
# PT-BR: Testes descartáveis do desktop empacotado; teclas nativas exigem opção explícita.
param(
 [Parameter(Mandatory=$true)][string]$Archive,
 [Parameter(Mandatory=$true)][string]$OutputDirectory,
 [ValidateSet('cairo','auto')][string]$Renderer='auto',
 [switch]$NativeKeys
)
$ErrorActionPreference='Stop'
$cases=@('desktop-input','login-input','multiline','edit-mentions','search-recycling','account-settings','voice-controls','symbolic-icons','status-viewer','windows-notifications','notification-window','notification-filters','voice-chat','native_voice_worker_drop_releases_process','native_call_worker_drop_closes_descendant_pipes','native_call_cues_complete_and_cancel_without_devices')
& (Join-Path $PSScriptRoot 'check-media-ui.ps1') -Archive $Archive -OutputDirectory $OutputDirectory -TestDirectory $PSScriptRoot -Renderer $Renderer -Cases $cases -DesktopSuite -NativeKeys:$NativeKeys
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
