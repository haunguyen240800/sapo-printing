# Disable WDAC (Windows Defender Application Control)
# Run this as Administrator

Write-Host "Disabling WDAC temporarily..." -ForegroundColor Cyan
Write-Host ""

try {
    # Method 1: Via Registry (requires restart)
    $regPath = "HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy"
    if (Test-Path $regPath) {
        Set-ItemProperty -Path $regPath -Name "VerifiedAndReputablePolicyState" -Value 0 -ErrorAction Stop
        Write-Host "SUCCESS: WDAC disabled via registry" -ForegroundColor Green
        Write-Host "IMPORTANT: You must RESTART Windows for this to take effect" -ForegroundColor Yellow
    }
} catch {
    Write-Host "Registry method failed: $($_.Exception.Message)" -ForegroundColor Red
}

Write-Host ""
Write-Host "Alternative: Disable via Group Policy Editor" -ForegroundColor Cyan
Write-Host "1. Press Win+R, type: gpedit.msc" -ForegroundColor White
Write-Host "2. Navigate to: Computer Config > Admin Templates > System > Device Guard" -ForegroundColor White
Write-Host "3. Set 'Turn On Virtualization Based Security' to Disabled" -ForegroundColor White
Write-Host "4. Restart Windows" -ForegroundColor White
Write-Host ""

Write-Host "After restart, run: pnpm dev" -ForegroundColor Green
Write-Host ""
Read-Host "Press Enter to exit"
