# Stop + delete the Sapo Printer Agent Windows Service.
# Run elevated by MSI/NSIS pre-uninstall hook. Safe if service is absent.
# IMPORTANT: keep this file ASCII-only (PowerShell 5.1 reads BOM-less scripts as ANSI).

$ErrorActionPreference = 'SilentlyContinue'
$Svc = 'SapoPrinterAgent'

$existing = Get-Service -Name $Svc -ErrorAction SilentlyContinue
if ($existing) {
    try { Stop-Service -Name $Svc -Force -ErrorAction SilentlyContinue } catch {}
    # Remove-Service is PS 6+, use sc.exe for Windows PowerShell 5.1 compatibility.
    & sc.exe delete $Svc | Out-Null
    Write-Host "Service $Svc removed"
}
exit 0
