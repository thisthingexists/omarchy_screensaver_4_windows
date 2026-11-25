# Builds dist\MyScreensaver.scr (plus the default art and license files).
#
#   powershell -ExecutionPolicy Bypass -File .\build.ps1          # build
#   powershell -ExecutionPolicy Bypass -File .\build.ps1 -Test    # run the effect test first
#
# Needs Rust (https://rustup.rs). Either Windows toolchain works:
#   x86_64-pc-windows-gnu  (self-contained, no Visual Studio needed)
#   x86_64-pc-windows-msvc (needs the VS Build Tools "Desktop development with C++")
param([switch]$Test)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$cargo = (Get-Command cargo -ErrorAction SilentlyContinue).Source
if (-not $cargo) { $cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe" }
if (-not (Test-Path $cargo)) { throw "cargo not found. Install Rust from https://rustup.rs" }

if ($Test) {
    & $cargo test --release -- --nocapture
    if ($LASTEXITCODE -ne 0) { throw "tests failed" }
}

& $cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

New-Item -ItemType Directory -Force dist | Out-Null
Copy-Item target\release\myscreensaver.exe dist\MyScreensaver.scr -Force
Copy-Item assets\screensaver.txt, LICENSE, THIRD_PARTY_NOTICES.md dist\ -Force

$scr = Get-Item dist\MyScreensaver.scr
"Built $($scr.FullName) ($([math]::Round($scr.Length / 1KB)) KB)"
