param([Parameter(Mandatory=$true)][string[]]$Paths,
      [string]$CertificateThumbprint,
      [string]$TimestampServer = 'http://timestamp.digicert.com',
      [switch]$RequireSignature)
$ErrorActionPreference = 'Stop'
foreach ($path in $Paths) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Arquivo ausente: $path" }
    if ($CertificateThumbprint) {
        $thumb = $CertificateThumbprint -replace '\s', ''
        if ($thumb -notmatch '^[0-9a-fA-F]{40}$') { throw 'Thumbprint inválido' }
        $cert = Get-Item -LiteralPath "Cert:\CurrentUser\My\$thumb"
        if (-not $cert.HasPrivateKey) { throw 'Certificado sem chave privada' }
        $result = Set-AuthenticodeSignature -LiteralPath $path -Certificate $cert -HashAlgorithm SHA256 -TimestampServer $TimestampServer
        if ($result.Status -ne 'Valid') { throw "Falha ao assinar: $path ($($result.Status))" }
    }
    $signature = Get-AuthenticodeSignature -LiteralPath $path
    if ($RequireSignature -and ($signature.Status -ne 'Valid' -or -not $signature.TimeStamperCertificate)) {
        throw "Publicação bloqueada: assinatura válida com carimbo de tempo obrigatória: $path"
    }
    if ($signature.Status -ne 'Valid') { Write-Warning "Artefato de teste sem assinatura válida: $path" }
    Get-FileHash -LiteralPath $path -Algorithm SHA256
}
