# PowerShell script to check database state
$dbPath = "$env:USERPROFILE\.sapo-printer\config.db"

Write-Host "Database path: $dbPath" -ForegroundColor Cyan

if (Test-Path $dbPath) {
    Write-Host "Database file exists" -ForegroundColor Green

    # Check if we can read it
    try {
        Add-Type -AssemblyName System.Data
        $conn = New-Object System.Data.SQLite.SQLiteConnection
        $conn.ConnectionString = "Data Source=$dbPath"
        $conn.Open()

        # Query recent jobs
        $cmd = $conn.CreateCommand()
        $cmd.CommandText = "SELECT id, status, retry_count, datetime(created_at, 'unixepoch', 'localtime') as created FROM print_jobs ORDER BY created_at DESC LIMIT 5"
        $reader = $cmd.ExecuteReader()

        Write-Host "`nRecent jobs:" -ForegroundColor Yellow
        $count = 0
        while ($reader.Read()) {
            $count++
            Write-Host "  [$count] ID: $($reader['id'])" -ForegroundColor White
            Write-Host "      Status: $($reader['status'])" -ForegroundColor Cyan
            Write-Host "      Retry: $($reader['retry_count'])" -ForegroundColor Gray
            Write-Host "      Created: $($reader['created'])" -ForegroundColor Gray
            Write-Host ""
        }

        if ($count -eq 0) {
            Write-Host "  No jobs found in database" -ForegroundColor Red
        }

        $reader.Close()
        $conn.Close()
    }
    catch {
        Write-Host "Error reading database (SQLite module may not be installed): $_" -ForegroundColor Red
        Write-Host "`nTry installing: Install-Module -Name System.Data.SQLite" -ForegroundColor Yellow
        Write-Host "Or use any SQLite browser tool to open: $dbPath" -ForegroundColor Yellow
    }
}
else {
    Write-Host "Database file NOT found at: $dbPath" -ForegroundColor Red
}

Write-Host "`nDatabase size:" -ForegroundColor Cyan
if (Test-Path $dbPath) {
    $size = (Get-Item $dbPath).Length
    Write-Host "  $size bytes" -ForegroundColor White
}
