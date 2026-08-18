# Register + start the Sapo Printer Agent Windows Service.
# Run elevated by MSI/NSIS post-install hook. Idempotent (safe on updates).
# IMPORTANT: keep this file ASCII-only (PowerShell 5.1 reads BOM-less scripts as ANSI).

$ErrorActionPreference = 'Stop'
$InstallDir = $PSScriptRoot
$Exe = Join-Path $InstallDir 'sapo-printer-cert-manager.exe'
$Svc = 'SapoPrinterAgent'

if (-not (Test-Path $Exe)) {
    Write-Error "Agent binary not found: $Exe"
    exit 1
}

# BinaryPathName: quote the exe path (spaces) and append the --service flag so the
# binary runs under the SCM dispatcher instead of CLI/daemon mode.
$Bin = '"' + $Exe + '" --service'

$existing = Get-Service -Name $Svc -ErrorAction SilentlyContinue
if ($existing) {
    # Service already registered (update path): stop so the new exe can take over.
    try { Stop-Service -Name $Svc -Force -ErrorAction SilentlyContinue } catch {}
} else {
    New-Service -Name $Svc -BinaryPathName $Bin -DisplayName 'Sapo Printer Agent' `
        -Description 'Sapo Printer certificate + silent auto-update agent.' `
        -StartupType Automatic | Out-Null
    Write-Host "Service $Svc created"
}

Start-Service -Name $Svc
Write-Host "Service $Svc started"
exit 0
