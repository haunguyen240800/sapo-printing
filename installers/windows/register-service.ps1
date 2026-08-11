# Install-time script - run elevated by MSI/NSIS custom action.
# Install CA cert into Windows trust store for sapo-printer-cert-manager.
# NOTE: Windows Service integration will be added after the binary implements the SCM protocol.
# IMPORTANT: keep this file ASCII-only. PowerShell 5.1 reads BOM-less scripts using the
# system ANSI codepage, so non-ASCII bytes get mangled and break parsing.

$ErrorActionPreference = 'Stop'
$InstallDir = $PSScriptRoot
$Agent = Join-Path $InstallDir 'sapo-printer-cert-manager.exe'

if (-not (Test-Path $Agent)) {
    Write-Error "Cert manager binary not found: $Agent"
    exit 1
}

# 1. Generate CA + install into LocalMachine\Root (idempotent).
& $Agent --install-ca
if ($LASTEXITCODE -ne 0) {
    Write-Error "CA install failed (exit $LASTEXITCODE)"
    exit $LASTEXITCODE
}
Write-Host "CA installed into LocalMachine\Root"
Write-Host "Done - app ready to serve HTTPS on local.mysapo.net:18901"
