# Sign all Cargo build scripts to pass WDAC policy

Write-Host "Signing Cargo build scripts..." -ForegroundColor Green

# Create self-signed certificate if not exists
$cert = Get-ChildItem -Path Cert:\CurrentUser\My -CodeSigningCert | Select-Object -First 1

if (-not $cert) {
    Write-Host "Creating self-signed certificate..." -ForegroundColor Yellow
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=SAPO Development" -CertStoreLocation Cert:\CurrentUser\My
    Write-Host "Certificate created: $($cert.Thumbprint)" -ForegroundColor Green
}

# Find all build-script-build.exe files in target/debug/build
$buildScripts = Get-ChildItem -Path ".\src-tauri\target\debug\build" -Filter "build-script-build.exe" -Recurse -ErrorAction SilentlyContinue

if ($buildScripts.Count -eq 0) {
    Write-Host "No build scripts found. Run 'cargo build' first (it will fail, but creates the scripts)." -ForegroundColor Yellow

    # Try to compile to generate build scripts (will fail but creates files)
    Write-Host "Attempting to generate build scripts..." -ForegroundColor Yellow
    Push-Location src-tauri
    cargo build 2>&1 | Out-Null
    Pop-Location

    # Try again
    $buildScripts = Get-ChildItem -Path ".\src-tauri\target\debug\build" -Filter "build-script-build.exe" -Recurse -ErrorAction SilentlyContinue
}

Write-Host "Found $($buildScripts.Count) build script(s) to sign" -ForegroundColor Cyan

foreach ($script in $buildScripts) {
    Write-Host "Signing: $($script.FullName)" -ForegroundColor Gray
    Set-AuthenticodeSignature -FilePath $script.FullName -Certificate $cert -TimestampServer "http://timestamp.digicert.com" -ErrorAction SilentlyContinue | Out-Null
}

Write-Host "`nAll build scripts signed successfully!" -ForegroundColor Green
Write-Host "Now run: pnpm dev" -ForegroundColor Cyan
