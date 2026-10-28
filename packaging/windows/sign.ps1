# EN-US: Sign/verify one owned PE using an explicitly selected non-exported Windows store key.
# PT-BR: Assina/verifica um PE próprio usando uma chave não exportada escolhida no repositório Windows.
param(
 [Parameter(Mandatory=$true)][string]$Path,
 [Parameter(Mandatory=$true)][string]$Receipt,
 [Parameter(Mandatory=$true)][ValidateSet('application','installer','uninstaller')][string]$Role,
 [string]$SignTool=$env:PAPO_SIGNTOOL,
 [switch]$VerifyOnly
)
$ErrorActionPreference='Stop'
if([Environment]::OSVersion.Platform -ne 'Win32NT'){throw 'Native Windows signing/verification is required'}
$thumbprint=$env:PAPO_SIGNING_THUMBPRINT
if($thumbprint -notmatch '^[0-9a-fA-F]{40}$'){throw 'Select an owner-provided certificate with PAPO_SIGNING_THUMBPRINT; no automatic certificate selection'}
if(!$SignTool -or ![IO.Path]::IsPathRooted($SignTool) -or !(Test-Path -LiteralPath $SignTool -PathType Leaf)){throw 'PAPO_SIGNTOOL must select an absolute Windows SDK signtool.exe path'}
$file=(Get-Item -LiteralPath $Path).FullName
$before=(Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
if(!$VerifyOnly){
 $certificate=Get-Item ('Cert:\CurrentUser\My\'+$thumbprint)
 if(!$certificate.HasPrivateKey -or $certificate.NotAfter -le [DateTime]::Now){throw 'Selected code-signing key is unavailable or expired'}
 $ekus=@($certificate.Extensions|Where-Object {$_ -is [Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]}|ForEach-Object {$_.EnhancedKeyUsages}|ForEach-Object {$_.Value})
 if($ekus -notcontains '1.3.6.1.5.5.7.3.3'){throw 'Selected certificate has no Code Signing EKU'}
 $url=$null
 if(![Uri]::TryCreate($env:PAPO_TIMESTAMP_URL,[UriKind]::Absolute,[ref]$url) -or $url.Scheme -ne 'https' -or $url.UserInfo -or $url.Query -or $url.Fragment){throw 'PAPO_TIMESTAMP_URL must be an HTTPS RFC3161 endpoint without credentials/query/fragment'}
 # EN-US: SHA-1 identifies the store certificate only; file and RFC3161 digests are SHA-256.
 # PT-BR: SHA-1 só identifica o certificado; resumos do arquivo e RFC3161 usam SHA-256.
 & $SignTool sign /q /sha1 $thumbprint /s My /fd SHA256 /tr $url.AbsoluteUri /td SHA256 $file
 if($LASTEXITCODE -ne 0){throw 'Code signing failed; no release receipt written'}
}
# EN-US: Warnings are failures, including missing timestamp/revocation/trust verification.
# PT-BR: Avisos são falhas, inclusive ausência de timestamp e problemas de revogação/confiança.
& $SignTool verify /q /pa /all /tw $file
if($LASTEXITCODE -ne 0){throw 'Authenticode verification failed or returned warnings'}
$signature=Get-AuthenticodeSignature -LiteralPath $file
if($signature.Status -ne 'Valid' -or !$signature.SignerCertificate -or !$signature.TimeStamperCertificate -or $signature.SignerCertificate.Thumbprint -ne $thumbprint){throw 'Signature is untrusted, untimestamped or belongs to another signer'}
$sha=[Security.Cryptography.SHA256]::Create()
try{$certHash=([BitConverter]::ToString($sha.ComputeHash($signature.SignerCertificate.RawData))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}
$report=[ordered]@{schema=1;status='pass';role=$Role;verified_utc=[DateTime]::UtcNow.ToString('o');before_sha256=$before;sha256=(Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant();signer_thumbprint=$thumbprint.ToLowerInvariant();signer_certificate_sha256=$certHash;timestamped=$true;trusted=$true;verify_only=[bool]$VerifyOnly}
$parent=Split-Path -Parent ([IO.Path]::GetFullPath($Receipt));New-Item -ItemType Directory -Force $parent|Out-Null
$report|ConvertTo-Json|Set-Content -LiteralPath $Receipt -Encoding UTF8
$global:LASTEXITCODE=0
