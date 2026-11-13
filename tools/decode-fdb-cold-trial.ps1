#Requires -Version 7.5
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateRange(1, 8)][int]$Trial,
    [Parameter(Mandatory)][string]$PythonPath
)

$ErrorActionPreference = 'Stop'
$python = (Resolve-Path -LiteralPath $PythonPath).Path
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$baseline = Join-Path $repo 'docs/benchmarks/cold-etw-link-20261107'
$main = Join-Path $repo 'docs/benchmarks/cold-etw-main-20261107'
$dir = (Resolve-Path -LiteralPath (Join-Path $main ('trial-{0:00}' -f $Trial))).Path
if (-not $dir.StartsWith($main + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
    ((Get-Item -LiteralPath $dir -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'Unexpected decoder output path'
}
$trace = Join-Path $dir 'trace.etl'
$csv = Join-Path $dir 'decoded.tmp.csv'
$summary = Join-Path $dir 'tracerpt-summary.txt'
if (-not (Test-Path -LiteralPath (Join-Path $dir 'admin-result.json') -PathType Leaf) -or
    -not (Test-Path -LiteralPath $trace -PathType Leaf) -or (Test-Path -LiteralPath $summary)) {
    throw 'Trial trace incomplete or already processed'
}
$files = @(Get-ChildItem -LiteralPath $baseline, $main -File -Recurse -Force)
$etlBytes = [long]($files | Where-Object Extension -eq '.etl' | Measure-Object Length -Sum).Sum
$used = [long]($files | Measure-Object Length -Sum).Sum
$cap = [Math]::Min(180MB, 512MB - $used - 32MB)
if ($cap -lt 8MB -or $etlBytes -gt 256MB) { throw 'Decoder disk reserve insufficient' }

# Establish ownership of the exact temporary file before tracerpt can overwrite it.
$owner = [IO.File]::Open($csv, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
$owner.Dispose()
$args = @('"' + $trace + '"', '-of', 'CSV', '-o', '"' + $csv + '"',
    '-summary', '"' + $summary + '"', '-y')
$decoder = Start-Process -FilePath (Join-Path $env:windir 'System32/tracerpt.exe') `
    -ArgumentList $args -WindowStyle Hidden -PassThru
try {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    while (-not $decoder.WaitForExit(100)) {
        if ((Get-Item -LiteralPath $csv).Length -ge $cap -or $timer.Elapsed.TotalSeconds -ge 75) {
            $decoder.Kill()
            $decoder.WaitForExit()
            throw 'Decoder stopped at the time or disk reserve'
        }
    }
    if ($decoder.ExitCode -ne 0 -or (Get-Item -LiteralPath $csv).Length -gt $cap) {
        throw 'Decoder exit or final byte budget failed'
    }
} finally {
    $decoder.Dispose()
}

$env:PYTHONUTF8 = '1'
& $python (Join-Path $PSScriptRoot 'analyze-fdb-cold-etw.py') $Trial
if ($LASTEXITCODE -ne 0) { throw 'Filtered evidence incomplete' }
$verdict = Get-Content -LiteralPath (Join-Path $dir 'verdict.json') -Raw | ConvertFrom-Json
$selected = Get-Item -LiteralPath (Join-Path $dir 'target-events.jsonl')
if ($verdict.trial -ne $Trial -or $selected.Length -le 0) { throw 'Filtered evidence incomplete' }
$item = Get-Item -LiteralPath $csv -Force
if ($item.FullName -cne $csv -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'Temporary CSV ownership check failed'
}
Remove-Item -LiteralPath $csv
