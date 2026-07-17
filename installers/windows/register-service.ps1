# Install-time script — chạy elevated bởi MSI/NSIS custom action.
# Cài CA + register Windows Service cho sapo-printer-agent.

$ErrorActionPreference = 'Stop'
$InstallDir = $PSScriptRoot
$Agent = Join-Path $InstallDir 'sapo-printer-agent.exe'

if (-not (Test-Path $Agent)) {
    Write-Error "Agent binary not found: $Agent"
    exit 1
}

# 1. Sinh CA + install vào LocalMachine\Root.
& $Agent --install-ca
if ($LASTEXITCODE -ne 0) {
    Write-Error "CA install failed (exit $LASTEXITCODE)"
    exit $LASTEXITCODE
}
Write-Host "CA installed into LocalMachine\Root"

# 2. Register Windows Service.
$ServiceName = 'SapoPrinterAgent'
$existing = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "Service $ServiceName exists, stopping first"
    Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
    sc.exe delete $ServiceName | Out-Null
}

$binPath = "`"$Agent`""
sc.exe create $ServiceName binPath= $binPath start= auto DisplayName= "Sapo Printer Agent"
if ($LASTEXITCODE -ne 0) {
    Write-Error "sc create failed"
    exit $LASTEXITCODE
}

sc.exe description $ServiceName "Local HTTPS agent + cert lifecycle for Sapo Printer."
sc.exe failure $ServiceName reset= 86400 actions= restart/60000/restart/60000/restart/60000

Start-Service -Name $ServiceName
Write-Host "Service $ServiceName started"
