# Run this script as Administrator to add Windows Defender exclusion

Write-Host "Adding Windows Defender exclusion for sapo-printer build directory..." -ForegroundColor Cyan
Write-Host ""

$targetPath = "E:\Source Code\SAPO\sapo-printing\src-tauri\target"

try {
    Add-MpPreference -ExclusionPath $targetPath
    Write-Host "SUCCESS: Exclusion added for $targetPath" -ForegroundColor Green
    Write-Host ""
    Write-Host "Current exclusions:" -ForegroundColor Cyan
    (Get-MpPreference).ExclusionPath | Where-Object { $_ -like "*sapo*" }
    Write-Host ""
    Write-Host "You can now run: pnpm dev" -ForegroundColor Green
} catch {
    Write-Host "ERROR: Failed to add exclusion" -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor Red
    Write-Host ""
    Write-Host "Please run this script as Administrator:" -ForegroundColor Yellow
    Write-Host "  Right-click PowerShell -> Run as Administrator" -ForegroundColor Yellow
    Write-Host "  Then run: .\add_defender_exclusion.ps1" -ForegroundColor Yellow
}

Write-Host ""
Read-Host "Press Enter to exit"
