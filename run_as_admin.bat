@echo off
echo ========================================
echo Run sapo-printer from Administrator
echo ========================================
echo.
echo This will run the app with elevated privileges to bypass WDAC.
echo.
pause

cd /d "E:\Source Code\SAPO\sapo-printing"
powershell -Command "Start-Process cmd -ArgumentList '/c cd /d E:\Source Code\SAPO\sapo-printing && pnpm dev' -Verb RunAs"
