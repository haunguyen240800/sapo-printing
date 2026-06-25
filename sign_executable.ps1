# Self-sign sapo-printer.exe to bypass WDAC
# Run this as Administrator

Write-Host "Creating self-signed certificate and signing executable..." -ForegroundColor Cyan
Write-Host ""

$exePath = "E:\Source Code\SAPO\sapo-printing\src-tauri\target\debug\sapo-printer.exe"

if (-not (Test-Path $exePath)) {
    Write-Host "ERROR: Executable not found at $exePath" -ForegroundColor Red
    Read-Host "Press Enter to exit"
    exit 1
}

try {
    # Create self-signed certificate
    Write-Host "1. Creating self-signed certificate..." -ForegroundColor Cyan
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=SAPO Printer Dev" -CertStoreLocation Cert:\CurrentUser\My -NotAfter (Get-Date).AddYears(5)
    Write-Host "   Certificate created: $($cert.Thumbprint)" -ForegroundColor Green

    # Export and import to Trusted Root (so Windows trusts it)
    Write-Host "2. Adding certificate to Trusted Root..." -ForegroundColor Cyan
    Export-Certificate -Cert $cert -FilePath "$env:TEMP\sapo-cert.cer" | Out-Null
    Import-Certificate -FilePath "$env:TEMP\sapo-cert.cer" -CertStoreLocation Cert:\LocalMachine\Root
    Write-Host "   Certificate trusted" -ForegroundColor Green

    # Sign the executable
    Write-Host "3. Signing executable..." -ForegroundColor Cyan
    Set-AuthenticodeSignature -FilePath $exePath -Certificate $cert -TimestampServer "http://timestamp.digicert.com"
    Write-Host "   Executable signed successfully" -ForegroundColor Green

    # Verify
    Write-Host ""
    Write-Host "4. Verifying signature..." -ForegroundColor Cyan
    $sig = Get-AuthenticodeSignature -FilePath $exePath
    Write-Host "   Status: $($sig.Status)" -ForegroundColor $(if ($sig.Status -eq 'Valid') { 'Green' } else { 'Yellow' })

    Write-Host ""
    Write-Host "SUCCESS! Executable is now signed." -ForegroundColor Green
    Write-Host "You can now run: pnpm dev" -ForegroundColor Green

} catch {
    Write-Host ""
    Write-Host "ERROR: $($_.Exception.Message)" -ForegroundColor Red
    Write-Host ""
    Write-Host "Make sure you're running PowerShell as Administrator" -ForegroundColor Yellow
}

Write-Host ""
Read-Host "Press Enter to exit"
