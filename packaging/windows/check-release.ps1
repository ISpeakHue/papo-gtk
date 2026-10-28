# EN-US: Retest exact signed files in an empty disposable native Windows account.
# PT-BR: Retesta arquivos assinados exatos em uma conta Windows nativa vazia e descartável.
param(
 [Parameter(Mandatory=$true)][string]$CandidateDirectory,
 [Parameter(Mandatory=$true)][string]$TestDirectory,
 [Parameter(Mandatory=$true)][string]$OutputDirectory,
 [switch]$DisposableAccount
)
$ErrorActionPreference='Stop'
if(!$DisposableAccount){throw 'Signed release checks require an empty disposable Windows account'}
$CandidateDirectory=(Get-Item $CandidateDirectory).FullName
$TestDirectory=(Get-Item $TestDirectory).FullName
New-Item -ItemType Directory -Force $OutputDirectory|Out-Null
$OutputDirectory=(Get-Item $OutputDirectory).FullName
$metadata=Get-Content (Join-Path $CandidateDirectory 'candidate.json') -Raw -Encoding UTF8|ConvertFrom-Json
$archive=Join-Path $CandidateDirectory $metadata.archive.name
$installer=Join-Path $CandidateDirectory $metadata.installer.name
$thumb=$env:PAPO_SIGNING_THUMBPRINT;$tool=$env:PAPO_SIGNTOOL
$report=[ordered]@{schema=1;status='running';archive=$metadata.archive;installer=$metadata.installer;signatures=@{}}
$work=$null
function Verify([string]$name,[string]$path,[string]$role){
 $env:PAPO_SIGNING_THUMBPRINT=$thumb;$env:PAPO_SIGNTOOL=$tool
 $receipt=Join-Path $OutputDirectory ('signature-'+$name.Replace('/','-')+'.json')
 & (Join-Path $PSScriptRoot 'sign.ps1') -Path $path -Receipt $receipt -Role $role -VerifyOnly
 if($LASTEXITCODE -ne 0){throw 'Final signature verification failed'}
 $value=Get-Content $receipt -Raw -Encoding UTF8|ConvertFrom-Json
 if($value.signer_certificate_sha256 -ne $metadata.signer_certificate_sha256){throw 'Unexpected final signer'}
 $report.signatures[$name]=$value
}
try {
 if(!$thumb -or !$tool){throw 'Select the public signer thumbprint and Windows SDK verification tool before starting release checks'}
 if((Get-FileHash $archive -Algorithm SHA256).Hash -ne $metadata.archive.sha256 -or (Get-FileHash $installer -Algorithm SHA256).Hash -ne $metadata.installer.sha256){throw 'Candidate artifacts changed'}
 # EN-US: Installer runs first because it intentionally refuses an existing Papo profile/registration.
 # PT-BR: O instalador roda primeiro porque recusa perfis/registros Papo preexistentes.
 & (Join-Path $PSScriptRoot 'check-installer.ps1') -Installer $installer -OutputDirectory (Join-Path $OutputDirectory 'installer') -DisposableAccount -RequireSignatures
 if($LASTEXITCODE -ne 0){throw 'Signed installer acceptance failed'}
 $uninstallReceipt=Get-Content (Join-Path $OutputDirectory 'installer/uninstaller-signature.json') -Raw -Encoding UTF8|ConvertFrom-Json
 $report.signatures['uninstaller']=$uninstallReceipt
 & (Join-Path $PSScriptRoot 'check-portable.ps1') -Archive $archive -OutputDirectory (Join-Path $OutputDirectory 'portable')
 if($LASTEXITCODE -ne 0){throw 'Signed portable acceptance failed'}
 & (Join-Path $PSScriptRoot 'check-media-ui.ps1') -Archive $archive -TestDirectory $TestDirectory -OutputDirectory (Join-Path $OutputDirectory 'media-cairo')
 if($LASTEXITCODE -ne 0){throw 'Signed Cairo media checks failed'}
 & (Join-Path $PSScriptRoot 'check-media-ui.ps1') -Archive $archive -TestDirectory $TestDirectory -OutputDirectory (Join-Path $OutputDirectory 'media-auto') -Renderer auto -Cases @('mov-media','backend-gifs','status-viewer','video-audio')
 if($LASTEXITCODE -ne 0){throw 'Signed automatic-renderer media checks failed'}
 # EN-US: The source-matched test artifact supplies the verified native key helper.
 # PT-BR: O artefato de testes da mesma revisão fornece o auxiliar verificado de teclas nativas.
 & (Join-Path $TestDirectory 'check-desktop-ui.ps1') -Archive $archive -OutputDirectory (Join-Path $OutputDirectory 'desktop') -NativeKeys
 if($LASTEXITCODE -ne 0){throw 'Signed desktop/lifecycle checks failed'}
 $work=Join-Path $env:TEMP ('Papo-signatures-'+[guid]::NewGuid().ToString('N'))
 Add-Type -AssemblyName System.IO.Compression.FileSystem
 [IO.Compression.ZipFile]::ExtractToDirectory($archive,$work)
 $package=(Get-ChildItem $work -Directory|Select-Object -First 1).FullName
 foreach($name in @('Papo.exe','Papo.com','runtime/bin/papo-gtk.exe','runtime/bin/papo-shell.exe','runtime/bin/papo-capture.dll')){Verify $name (Join-Path $package $name) 'application'}
 Verify 'installer' $installer 'installer'
 $report.status='pass'
}catch{$report.status='fail';$report.error=$_.Exception.Message}
finally{
 if($work){Remove-Item $work -Recurse -Force}
 $report|ConvertTo-Json -Depth 10|Set-Content (Join-Path $OutputDirectory 'signed-recheck.json') -Encoding UTF8
}
if($report.status -ne 'pass'){throw "Signed candidate failed: $($report.error)"}
$global:LASTEXITCODE=0
