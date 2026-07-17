# Uninstall-time script — gỡ service + xóa CA khỏi trust store.

$ErrorActionPreference = 'Continue'
$InstallDir = $PSScriptRoot
$Agent = Join-Path $InstallDir 'sapo-printer-agent.exe'
$ServiceName = 'SapoPrinterAgent'

# 1. Stop + remove service.
$svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($svc) {
    Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
    sc.exe delete $ServiceName | Out-Null
    Write-Host "Service $ServiceName removed"
}

# 2. Uninstall CA.
if (Test-Path $Agent) {
    & $Agent --uninstall-ca
    if ($LASTEXITCODE -eq 0) {
        Write-Host "CA removed from LocalMachine\Root"
    }
}
