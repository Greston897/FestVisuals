@echo on
for /f "usebackq tokens=*" %%i in (`"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set VS=%%i
echo VS=%VS%
if not defined VS exit /b 1
call "%VS%\VC\Auxiliary\Build\vcvars64.bat"
if errorlevel 1 exit /b 1
git clone --depth 1 --branch VER-2-13-3 https://github.com/freetype/freetype.git "%RUNNER_TEMP%\freetype-src"
if errorlevel 1 exit /b 1
cmake -S "%RUNNER_TEMP%\freetype-src" -B "%RUNNER_TEMP%\freetype-build" -G Ninja -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DFT_DISABLE_ZLIB=TRUE -DFT_DISABLE_BZIP2=TRUE -DFT_DISABLE_PNG=TRUE -DFT_DISABLE_HARFBUZZ=TRUE -DFT_DISABLE_BROTLI=TRUE
if errorlevel 1 exit /b 1
cmake --build "%RUNNER_TEMP%\freetype-build"
if errorlevel 1 exit /b 1
set FREETYPE_INCLUDE=%RUNNER_TEMP%\freetype-build\include;%RUNNER_TEMP%\freetype-src\include
set FREETYPE_LIB=
for /f "delims=" %%i in ('dir /s /b "%RUNNER_TEMP%\freetype-build\freetype.lib"') do set FREETYPE_LIB=%%i
if not defined FREETYPE_LIB exit /b 1
for %%i in ("%FREETYPE_LIB%") do set FREETYPE_LIB_DIR=%%~dpinose
set FREETYPE_LIB_DIR=%FREETYPE_LIB_DIR:~0,-4%
echo FREETYPE_LIB=%FREETYPE_LIB%
echo FREETYPE_LIB_DIR=%FREETYPE_LIB_DIR%
cargo build --release --manifest-path launcher-rs\Cargo.toml
if errorlevel 1 exit /b 1
copy /Y launcher-rs\target\release\festvisuals.exe FestVisuals.exe
if errorlevel 1 exit /b 1
exit /b 0
