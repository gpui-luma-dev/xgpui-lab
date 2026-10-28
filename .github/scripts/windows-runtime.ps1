param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$vswhere = "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe"
if (-not (Test-Path $vswhere)) { throw 'Visual Studio locator is missing' }
$binaries = @(& $vswhere -latest -products '*' -find 'VC/Tools/MSVC/**/bin/Host*/*/dumpbin.exe')
if ($LASTEXITCODE -ne 0) { throw 'Could not locate Visual Studio tools' }
$preferred = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'Hostarm64[/\\]arm64' } else { 'Hostx64[/\\]x64' }
$dumpbin = $binaries | Where-Object { $_ -match $preferred } | Select-Object -First 1
if (-not $dumpbin) { throw "No native dumpbin found ($preferred)" }
$report = @(& $dumpbin /DEPENDENTS $Executable)
if ($LASTEXITCODE -ne 0) { throw 'dumpbin dependency inspection failed' }
$report | Set-Content -Encoding utf8 (Join-Path $OutputDirectory 'dll-dependencies.txt')
$dlls = @($report | ForEach-Object {
    if ($_ -match '^\s+([A-Za-z0-9_.-]+\.dll)\s*$') { $Matches[1] }
} | Sort-Object -Unique)
if ($dlls.Count -eq 0) { throw 'No DLL imports found; refusing an unverified runtime report' }
foreach ($dll in $dlls) {
    if ($dll -match '^(vcruntime|msvcp|concrt)') {
        throw "Unexpected VC runtime import $dll; the archive build must link its CRT statically"
    }
    if ($dll -notmatch '^(api-ms-win-|ext-ms-win-)' -and -not (Test-Path (Join-Path "$env:windir/System32" $dll))) {
        throw "Non-system runtime dependency $dll needs an explicit packaging decision"
    }
}
@{ imported_dlls = $dlls; crt = 'static'; inspection = 'dumpbin /DEPENDENTS';
   graphics_smoke_test = 'not performed; requires a Windows desktop and supported GPU' } |
    ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8 (Join-Path $OutputDirectory 'runtime.json')
Write-Output "Runtime imports inspected: $($dlls -join ', ')"
