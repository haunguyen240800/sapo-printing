# Stop + delete the Sapo Printer Agent Windows Service.
# Run elevated by MSI/NSIS pre-uninstall hook. Safe if service is absent.
# IMPORTANT: keep this file ASCII-only (PowerShell 5.1 reads BOM-less scripts as ANSI).
# Default mirrors src-tauri/src/infrastructure/platform/agent_config.rs, the canonical Rust source.

param(
    [ValidateNotNullOrEmpty()]
    [ValidatePattern('^[A-Za-z0-9_.-]+$')]
    [string]$ServiceName = 'SapoPrinterAgent'
)

$ErrorActionPreference = 'SilentlyContinue'

$existing = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($existing) {
    try { Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue } catch {}
    # Remove-Service is PS 6+, use sc.exe for Windows PowerShell 5.1 compatibility.
    & sc.exe delete $ServiceName | Out-Null
    Write-Host "Service $ServiceName removed"
}
exit 0
