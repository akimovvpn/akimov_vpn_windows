$ErrorActionPreference = 'Stop'
$cache = Join-Path (Split-Path (Split-Path $PSScriptRoot -Parent) -Parent) '.tools'
$fixture = Join-Path $cache 'interop'
New-Item -ItemType Directory -Force $fixture | Out-Null
$archive = Join-Path $cache 'xray-windows-64.zip'
if (!(Test-Path -LiteralPath $archive)) {
    Invoke-WebRequest 'https://github.com/XTLS/Xray-core/releases/download/v26.10.31/Xray-windows-64.zip' -OutFile $archive
}
if ((Get-FileHash -LiteralPath $archive).Hash -ne '2E04A9995A6F2AD1B28C0BB25C708ECB2695E43A1C9C7889C4B6AC2B48CF7A6A') { throw 'Xray fixture checksum mismatch.' }
Expand-Archive -LiteralPath $archive -DestinationPath (Join-Path $cache 'xray') -Force
$rsa = [System.Security.Cryptography.RSA]::Create(2048)
try {
    $request = [System.Security.Cryptography.X509Certificates.CertificateRequest]::new('CN=localhost', $rsa, [System.Security.Cryptography.HashAlgorithmName]::SHA256, [System.Security.Cryptography.RSASignaturePadding]::Pkcs1)
    $san = [System.Security.Cryptography.X509Certificates.SubjectAlternativeNameBuilder]::new()
    $san.AddDnsName('localhost')
    $request.CertificateExtensions.Add($san.Build())
    $certificate = $request.CreateSelfSigned([DateTimeOffset]::Now.AddMinutes(-5), [DateTimeOffset]::Now.AddDays(2))
    try {
        [IO.File]::WriteAllText((Join-Path $fixture 'cert.pem'), $certificate.ExportCertificatePem())
        [IO.File]::WriteAllText((Join-Path $fixture 'key.pem'), $rsa.ExportPkcs8PrivateKeyPem())
    } finally { $certificate.Dispose() }
} finally { $rsa.Dispose() }
Write-Host 'Prepared local Xray fixture; no certificate installed or trusted.'
