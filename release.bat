@echo off
setlocal

if "%~1"=="" (
    echo Usage: release.bat ^<version^>
    echo Example: release.bat 0.2.0
    exit /b 1
)

set VERSION=%~1

:: Strip leading "v" if provided
if "%VERSION:~0,1%"=="v" set VERSION=%VERSION:~1%

echo Tagging v%VERSION% and pushing...
git tag "v%VERSION%" || exit /b 1
git push origin "v%VERSION%" || exit /b 1

echo.
echo Release v%VERSION% pushed. GitHub Actions will build and publish.
echo https://github.com/your-username/context-menu-helper/actions
