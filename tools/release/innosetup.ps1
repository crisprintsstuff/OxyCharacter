# Build the Windows installer. Called from package.sh on the GitHub runner.
$ErrorActionPreference = 'Stop'
$version = $env:OXY_ISS_VERSION
$source = $env:OXY_ISS_SOURCE
$out = $env:OXY_ISS_OUT
$script = $env:OXY_ISS_SCRIPT
if (-not $version -or -not $source -or -not $out -or -not $script) {
    throw 'OXY_ISS_VERSION, OXY_ISS_SOURCE, OXY_ISS_OUT and OXY_ISS_SCRIPT must be set'
}
$candidates = @(
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
)
$iscc = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) {
    throw 'ISCC.exe not found. Install Inno Setup 6.'
}
Write-Host "Using $iscc"
& $iscc "/DMyVersion=$version" "/DSourceDir=$source" "/DOutDir=$out" $script
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
