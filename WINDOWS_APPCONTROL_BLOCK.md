# Windows Application Control Block - Solution

## Issue

```
error: could not execute process `target\debug\sapo-printer.exe` (never executed)
Caused by:
  An Application Control policy has blocked this file. (os error 4551)
```

Windows Defender Application Control is blocking the executable.

## Solution

### Option 1: Add Windows Defender Exclusion (Recommended)

**Run as Administrator:**

1. Right-click **PowerShell** → **Run as Administrator**
2. Run:
   ```powershell
   cd "E:\Source Code\SAPO\sapo-printing"
   .\add_defender_exclusion.ps1
   ```

Or manually:
```powershell
Add-MpPreference -ExclusionPath "E:\Source Code\SAPO\sapo-printing\src-tauri\target"
```

### Option 2: Disable Real-time Protection Temporarily

**Windows Security → Virus & threat protection → Manage settings:**
- Turn off "Real-time protection" temporarily
- Run `pnpm dev`
- Turn it back on after testing

### Option 3: Sign the Executable (Production)

For production builds, sign the executable with a code signing certificate to avoid SmartScreen/AppControl blocks.

## Verification

After adding exclusion:
```powershell
# Verify exclusion was added
(Get-MpPreference).ExclusionPath | Where-Object { $_ -like "*sapo*" }
```

Expected output:
```
E:\Source Code\SAPO\sapo-printing\src-tauri\target
```

Then run:
```bash
pnpm dev
```

## Alternative: Run from Different Location

If you can't modify Defender settings, copy the project to a different drive (e.g., D:\ or C:\Dev\) which may have different AppControl policies.

## Why This Happens

- Unsigned executables in certain paths trigger Windows SmartScreen
- E:\ drive external/network drives have stricter policies
- Development builds are not signed by default
- Windows 11 has stricter AppControl than Windows 10

## For CI/CD

Add exclusions in build scripts:
```yaml
- name: Add Defender Exclusion
  shell: powershell
  run: |
    Add-MpPreference -ExclusionPath "${{ github.workspace }}\src-tauri\target"
```

## Next Steps

1. **Add Defender exclusion** using the script above
2. **Run `pnpm dev`** again
3. **Test print flow** with logs
4. Should see WindowsPrinterEngine logs with actual printing
