# Register + start the Sapo Printer Agent Windows Service.
# Run elevated by MSI/NSIS post-install hook. Idempotent (safe on updates).
# IMPORTANT: keep this file ASCII-only (PowerShell 5.1 reads BOM-less scripts as ANSI).
# Defaults mirror src-tauri/src/infrastructure/platform/agent_config.rs, the canonical Rust source.

param(
    [ValidateNotNullOrEmpty()]
    [ValidatePattern('^[A-Za-z0-9_.-]+$')]
    [string]$ServiceName = 'SapoPrinterAgent',

    [ValidateNotNullOrEmpty()]
    [ValidatePattern('^[A-Za-z0-9_.-]+\.exe$')]
    [string]$AgentExeName = 'sapo-printer-cert-manager.exe'
)

$ErrorActionPreference = 'Stop'
$InstallDir = $PSScriptRoot
$Exe = Join-Path $InstallDir $AgentExeName

if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) {
    Write-Error "Agent binary not found: $Exe"
    exit 1
}

# BinaryPathName: quote the exe path (spaces) and append the --service flag so the
# binary runs under the SCM dispatcher instead of CLI/daemon mode.
$Bin = '"' + $Exe + '" --service'

$existing = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($existing) {
    # Service already registered (update path): stop so the new exe can take over.
    try { Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue } catch {}
    & sc.exe config $ServiceName binPath= $Bin | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to update service $ServiceName binary path (exit $LASTEXITCODE)"
    }
} else {
    New-Service -Name $ServiceName -BinaryPathName $Bin -DisplayName 'Sapo Printer Agent' `
        -Description 'Sapo Printer certificate + silent auto-update agent.' `
        -StartupType Automatic | Out-Null
    Write-Host "Service $ServiceName created"
}

Start-Service -Name $ServiceName
Write-Host "Service $ServiceName started"
exit 0
