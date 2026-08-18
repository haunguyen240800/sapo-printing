# Register + start the Sapo Printer Pro Max Agent Windows Service.
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
$ServiceDisplayName = 'Sapo Printer Pro Max Agent'
$ServiceDescription = 'Sapo Printer Pro Max certificate + silent auto-update agent.'

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
    try {
        & sc.exe config $ServiceName binPath= $Bin | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to update service $ServiceName binary path (exit $LASTEXITCODE)"
        }
        & sc.exe config $ServiceName DisplayName= $ServiceDisplayName | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to update service $ServiceName display name (exit $LASTEXITCODE)"
        }
        & sc.exe description $ServiceName $ServiceDescription | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to update service $ServiceName description (exit $LASTEXITCODE)"
        }
    } finally {
        # A metadata-only failure must not leave the existing functional service stopped.
        Start-Service -Name $ServiceName
        Write-Host "Service $ServiceName started"
    }
} else {
    New-Service -Name $ServiceName -BinaryPathName $Bin -DisplayName $ServiceDisplayName `
        -Description $ServiceDescription `
        -StartupType Automatic | Out-Null
    Write-Host "Service $ServiceName created"
    Start-Service -Name $ServiceName
    Write-Host "Service $ServiceName started"
}
exit 0
