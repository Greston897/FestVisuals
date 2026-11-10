$ErrorActionPreference = 'Stop'
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw 'vswhere found no Visual Studio' }
Write-Host "VS=$vs"
cmd /c "`"$vs\VC\Auxiliary\Build\vcvars64.bat`" >nul && set" | ForEach-Object {
    if ($_ -match '^(.*?)=(.*)$') {
        Set-Item -Path "Env:$($matches[1])" -Value $matches[2]
    }
}
Write-Host "cmake=$(Get-Command cmake | Select-Object -ExpandProperty Source)"
Write-Host "ninja=$(Get-Command ninja | Select-Object -ExpandProperty Source)"
Write-Host "cargo=$(Get-Command cargo | Select-Object -ExpandProperty Source)"
$src = Join-Path $env:RUNNER_TEMP 'freetype-src'
$build = Join-Path $env:RUNNER_TEMP 'freetype-build'
git clone --depth 1 --branch VER-2-13-3 https://github.com/freetype/freetype.git $src
if ($LASTEXITCODE -ne 0) { throw 'git clone failed' }
cmake -S $src -B $build -G Ninja -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DFT_DISABLE_ZLIB=TRUE -DFT_DISABLE_BZIP2=TRUE -DFT_DISABLE_PNG=TRUE -DFT_DISABLE_HARFBUZZ=TRUE -DFT_DISABLE_BROTLI=TRUE
if ($LASTEXITCODE -ne 0) { throw 'cmake configure failed' }
cmake --build $build
if ($LASTEXITCODE -ne 0) { throw 'cmake build failed' }
$lib = Get-ChildItem -Path $build -Recurse -Filter freetype.lib | Select-Object -First 1
if (-not $lib) { throw 'freetype.lib not found' }
$env:FREETYPE_LIB_DIR = $lib.DirectoryName
$env:FREETYPE_INCLUDE = "$build\include;$src\include"
Write-Host "FREETYPE_LIB=$($lib.FullName)"
Write-Host "FREETYPE_INCLUDE=$env:FREETYPE_INCLUDE"
cargo build --release --manifest-path launcher-rs\Cargo.toml
if ($LASTEXITCODE -ne 0) { throw 'cargo failed' }
Copy-Item launcher-rs\target\release\festvisuals.exe FestVisuals.exe -Force
Write-Host "exe=$((Get-Item FestVisuals.exe).Length) bytes"
