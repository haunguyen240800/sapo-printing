# Uninstall-time script - remove CA cert from Windows trust store.
# IMPORTANT: keep this file ASCII-only (see install-ca-cert.ps1 for the reason).

$ErrorActionPreference = 'Continue'
$InstallDir = $PSScriptRoot
$Agent = Join-Path $InstallDir 'sapo-printer-cert-manager.exe'

# Uninstall CA.
if (Test-Path $Agent) {
    & $Agent --uninstall-ca
    if ($LASTEXITCODE -eq 0) {
        Write-Host "CA removed from LocalMachine\Root"
    }
}
