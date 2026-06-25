@echo off
REM Script to download and install PDFium DLL for Windows

echo ========================================
echo PDFium DLL Installer for sapo-printer
echo ========================================
echo.

set "TARGET_DIR=E:\Source Code\SAPO\sapo-printing\src-tauri\target\debug"
set "DOWNLOAD_URL=https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F6666/pdfium-win-x64.tgz"
set "TEMP_FILE=%TEMP%\pdfium-win-x64.tgz"

echo Step 1: Downloading PDFium binary...
echo URL: %DOWNLOAD_URL%
echo.

curl -L -o "%TEMP_FILE%" "%DOWNLOAD_URL%"

if %ERRORLEVEL% NEQ 0 (
    echo ERROR: Failed to download PDFium binary
    echo Please download manually from:
    echo https://github.com/bblanchon/pdfium-binaries/releases
    pause
    exit /b 1
)

echo Step 2: Extracting pdfium.dll...
echo.

REM Extract .tgz using tar (available in Windows 10+)
tar -xzf "%TEMP_FILE%" -C "%TEMP%" bin/pdfium.dll 2>nul

if %ERRORLEVEL% NEQ 0 (
    echo ERROR: Failed to extract archive
    echo You may need to extract manually using 7-Zip or WinRAR
    echo.
    echo Downloaded file location: %TEMP_FILE%
    echo Extract bin/pdfium.dll to: %TARGET_DIR%
    pause
    exit /b 1
)

echo Step 3: Copying pdfium.dll to target directory...
echo Target: %TARGET_DIR%
echo.

if not exist "%TARGET_DIR%" (
    echo Creating target directory...
    mkdir "%TARGET_DIR%"
)

copy /Y "%TEMP%\bin\pdfium.dll" "%TARGET_DIR%\pdfium.dll"

if %ERRORLEVEL% NEQ 0 (
    echo ERROR: Failed to copy DLL
    pause
    exit /b 1
)

echo Step 4: Verifying installation...
echo.

if exist "%TARGET_DIR%\pdfium.dll" (
    echo SUCCESS! pdfium.dll installed successfully
    echo Location: %TARGET_DIR%\pdfium.dll
    echo.
    echo File info:
    dir "%TARGET_DIR%\pdfium.dll"
) else (
    echo ERROR: pdfium.dll not found after installation
    exit /b 1
)

echo.
echo Step 5: Cleanup...
del "%TEMP_FILE%" 2>nul
rmdir /S /Q "%TEMP%\bin" 2>nul

echo.
echo ========================================
echo Installation complete!
echo ========================================
echo.
echo Next steps:
echo 1. Run: cargo run --manifest-path=src-tauri/Cargo.toml
echo 2. Create a test print job
echo 3. Check logs for successful PDF rendering
echo.
pause
