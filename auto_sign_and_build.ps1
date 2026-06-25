# Auto-sign build scripts in a loop until build succeeds

Write-Host "Starting auto-sign and build loop..." -ForegroundColor Green

# Get or create certificate
$cert = Get-ChildItem -Path Cert:\CurrentUser\My -CodeSigningCert | Select-Object -First 1
if (-not $cert) {
    Write-Host "Creating self-signed certificate..." -ForegroundColor Yellow
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=SAPO Development" -CertStoreLocation Cert:\CurrentUser\My

    # Trust the certificate
    Write-Host "Adding certificate to Trusted Root..." -ForegroundColor Yellow
    $store = New-Object System.Security.Cryptography.X509Certificates.X509Store("Root", "CurrentUser")
    $store.Open("ReadWrite")
    $store.Add($cert)
    $store.Close()
}

$maxIterations = 20
$iteration = 0

while ($iteration -lt $maxIterations) {
    $iteration++
    Write-Host "`n=== Iteration $iteration ===" -ForegroundColor Cyan

    # Sign all build scripts
    $buildScripts = Get-ChildItem -Path ".\src-tauri\target\debug\build" -Filter "build-script-build.exe" -Recurse -ErrorAction SilentlyContinue

    if ($buildScripts.Count -gt 0) {
        Write-Host "Signing $($buildScripts.Count) build script(s)..." -ForegroundColor Yellow
        foreach ($script in $buildScripts) {
            Set-AuthenticodeSignature -FilePath $script.FullName -Certificate $cert -ErrorAction SilentlyContinue | Out-Null
        }
        Write-Host "Signed all build scripts" -ForegroundColor Green
    }

    # Try to build
    Write-Host "Running cargo build..." -ForegroundColor Yellow
    Push-Location src-tauri
    $buildOutput = cargo build 2>&1
    $buildExitCode = $LASTEXITCODE
    Pop-Location

    if ($buildExitCode -eq 0) {
        Write-Host "`nBuild succeeded!" -ForegroundColor Green
        break
    }

    # Check if still blocked
    $isBlocked = $buildOutput | Select-String -Pattern "os error 4551"
    if (-not $isBlocked) {
        Write-Host "`nBuild failed with different error:" -ForegroundColor Red
        $buildOutput | Select-String -Pattern "error:" | Select-Object -First 5
        break
    }

    Write-Host "Still blocked, signing new scripts..." -ForegroundColor Yellow
    Start-Sleep -Seconds 1
}

if ($iteration -ge $maxIterations) {
    Write-Host "`nReached max iterations. Manual intervention needed." -ForegroundColor Red
} else {
    Write-Host "`nReady to run: pnpm dev" -ForegroundColor Cyan
}
