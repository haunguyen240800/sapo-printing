# Manual release helper for Windows PowerShell.
#
# What it does:
#   1. Reads the product name and version from src-tauri/tauri.conf.json
#   2. Locates the NSIS installer + its .sig produced by `pnpm build`
#   3. Generates latest.json (the updater manifest) with the correct
#      signature content and GitHub download URL
#   4. Creates the GitHub release (if `gh` is installed), otherwise prints
#      manual upload instructions
#
# Usage (from the repo root):
#   powershell -ExecutionPolicy Bypass -File scripts\release.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\release.ps1 -Publish

param(
    [switch]$Publish
)

$ErrorActionPreference = "Stop"

# --- Config -----------------------------------------------------------------
$Repo = "haunguyen240800/sapo-printing"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Root = Split-Path -Parent $ScriptDir

$Conf = Join-Path $Root "src-tauri\tauri.conf.json"
$NsisDir = Join-Path $Root "src-tauri\target\release\bundle\nsis"

# --- 1. Read product metadata -----------------------------------------------
if (-not (Test-Path $Conf)) { throw "Cannot find $Conf" }
$TauriConfig = Get-Content $Conf -Raw | ConvertFrom-Json
$ProductName = [string]$TauriConfig.productName
$Version = [string]$TauriConfig.version
if ([string]::IsNullOrWhiteSpace($ProductName)) {
    throw "Could not parse productName from tauri.conf.json"
}
if ([string]::IsNullOrWhiteSpace($Version)) {
    throw "Could not parse version from tauri.conf.json"
}
$Tag = "v$Version"
Write-Host ">> Product: $ProductName"
Write-Host ">> Version: $Version  (tag: $Tag)"

# --- 2. Locate installer + signature ---------------------------------------
$Exe = Join-Path $NsisDir "${ProductName}_${Version}_x64-setup.exe"
$Sig = "$Exe.sig"

if (-not (Test-Path $Exe)) {
    throw "Installer not found: $Exe`n       Did you run 'pnpm build' after bumping the version?"
}
if (-not (Test-Path $Sig)) {
    throw "Signature not found: $Sig`n       Did you set TAURI_SIGNING_PRIVATE_KEY before building?"
}
Write-Host ">> Installer: $Exe"
Write-Host ">> Signature: $Sig"

# --- 3. Generate latest.json ------------------------------------------------
# GitHub replaces spaces in asset filenames with dots in the download URL.
$AssetName = Split-Path -Leaf $Exe
$UrlName = $AssetName -replace " ", "."
$DownloadUrl = "https://github.com/$Repo/releases/download/$Tag/$UrlName"

$Signature = [string](Get-Content $Sig -Raw)
$Signature = $Signature.Trim()
$PubDate = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

$Manifest = [ordered]@{
    version   = $Version
    notes     = "Release $Tag"
    pub_date  = $PubDate
    platforms = [ordered]@{
        "windows-x86_64" = [ordered]@{
            signature = $Signature
            url       = $DownloadUrl
        }
    }
}

$Out = Join-Path $NsisDir "latest.json"
$Manifest | ConvertTo-Json -Depth 5 | Out-File -FilePath $Out -Encoding utf8
Write-Host ">> Wrote manifest: $Out"
Write-Host ">> Download URL:   $DownloadUrl"
Write-Host "--------------------------------------------------------------------"
Get-Content $Out -Raw | Write-Host
Write-Host "--------------------------------------------------------------------"

# --- 4. Publish -------------------------------------------------------------
if ($Publish) {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
        throw "gh CLI not installed. Install with: winget install --id GitHub.cli`n       Then run: gh auth login"
    }
    Write-Host ">> Publishing release..."
    gh release create $Tag $Exe $Out --repo $Repo --title $Tag --notes "Release $Tag"
    Write-Host ">> Done. Release $Tag created."
} else {
    Write-Host ">> Dry run complete. To publish, either:"
    Write-Host "   A) Run: powershell -ExecutionPolicy Bypass -File scripts\release.ps1 -Publish   (needs gh CLI)"
    Write-Host "   B) Manually create release $Tag at:"
    Write-Host "      https://github.com/$Repo/releases/new?tag=$Tag"
    Write-Host "      and upload these two files as assets:"
    Write-Host "        - $Exe"
    Write-Host "        - $Out"
}
