#Requires -Version 7.5
# Normal-user benchmark side of the administrator handoff.
[CmdletBinding()]
param([Parameter(Mandatory)][ValidateRange(1, 8)][int]$Trial)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$output = Join-Path $repo 'docs/benchmarks/cold-etw-main-20261107'
$run = Get-Content -LiteralPath (Join-Path $output 'run.json') -Raw | ConvertFrom-Json -DateKind String
$engine = $run.order[$Trial - 1]
$start = [DateTimeOffset]::Parse($run.start, [cultureinfo]::InvariantCulture)
$stop = $start.AddMinutes(17)
$trialDir = Join-Path $output ('trial-{0:00}' -f $Trial)
if (Test-Path -LiteralPath $trialDir) { throw 'Trial already claimed' }
foreach ($name in @('fdb', 'sqlite')) {
    $before = Get-Content -LiteralPath (Join-Path $output "$name-before.jsonl") -Raw | ConvertFrom-Json
    if ($before.oracle -ne $true -or $before.gets -ne 50000 -or $before.cases -ne 41) {
        throw 'Pre-trial oracle incomplete'
    }
}
$program = if ($engine -eq 'fdb') {
    Join-Path $repo 'target/release/examples/measure_cold_case.exe'
} else {
    Join-Path $repo 'tools/native_sqlite/target/release/measure_cold_case.exe'
}
$model = $run.files.$engine.model
$oracle = Join-Path $repo "docs/benchmarks/block3a-repair-20261103-$engine-recovered.jsonl"
$signal = 'Local\FDBColdMain' + $run.run_id + ('T{0:00}' -f $Trial)
$go = $null
$done = $null

function Run-Phase([string]$Mode) {
    $file = Join-Path $trialDir "$Mode.jsonl"
    $errorFile = Join-Path $trialDir "$Mode.stderr"
    if ((Test-Path -LiteralPath $file) -or (Test-Path -LiteralPath $errorFile)) {
        throw 'Phase output already exists'
    }
    $process = Start-Process -FilePath $program -WorkingDirectory $repo `
        -ArgumentList @($Mode, ('"' + $model + '"'), ('"' + $oracle + '"')) `
        -RedirectStandardOutput $file -RedirectStandardError $errorFile -WindowStyle Hidden -PassThru
    try {
        while (-not $process.WaitForExit(100)) {
            if ($phaseTimer.Elapsed.TotalSeconds -ge 45 -or [DateTimeOffset]::UtcNow -ge $stop) {
                $process.Kill()
                $process.WaitForExit()
                throw 'Reader phase timed out'
            }
        }
        if ($process.ExitCode -ne 0) { throw "Reader $Mode exit $($process.ExitCode)" }
        $row = Get-Content -LiteralPath $file -Raw | ConvertFrom-Json -DateKind String
        if ($row.oracle -ne $true -or $row.pid -ne $process.Id -or $row.engine -cne $engine -or
            $row.mode -cne $Mode -or $row.sequence -ne $run.files.$engine.sequence) {
            throw 'Unexpected phase result'
        }
        $row
    } finally {
        $process.Dispose()
    }
}

try {
    $created = $false
    $go = [Threading.EventWaitHandle]::new($false, [Threading.EventResetMode]::AutoReset, $signal + 'Go', [ref]$created)
    if (-not $created) { throw 'Go signal already exists' }
    $done = [Threading.EventWaitHandle]::new($false, [Threading.EventResetMode]::AutoReset, $signal + 'Done', [ref]$created)
    if (-not $created) { throw 'Done signal already exists' }
    $arm = [ordered]@{version=1; run_id=$run.run_id; trial=$Trial; engine=$engine; anchor='tr-case0';
        start_utc=$run.start; deadline_utc=$run.deadline}
    $arm | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $output 'arm.json')
    [pscustomobject]@{status='armed'; trial=$Trial; engine=$engine; pid=$PID; utc=[DateTimeOffset]::UtcNow.ToString('o')} | ConvertTo-Json -Compress
    $wait = [int][Math]::Max(0, ($stop.AddSeconds(-15) - [DateTimeOffset]::UtcNow).TotalMilliseconds)
    if (-not $go.WaitOne($wait)) { throw 'Purge signal did not arrive inside the trial window' }
    $phaseTimer = [Diagnostics.Stopwatch]::StartNew()
    $cold = Run-Phase 'cold'
    $buffer = [byte[]]::new(1MB)
    $warmed = 0L
    foreach ($entry in $run.files.$engine.entries) {
        if ($entry.length -eq 0) { continue }
        if ($phaseTimer.Elapsed.TotalSeconds -ge 40) { throw 'Warm phase reserve exhausted' }
        $reader = [IO.File]::OpenRead($entry.path)
        try {
            if ($reader.Length -ne $entry.length) { throw 'Target inventory changed' }
            while (($n = $reader.Read($buffer, 0, $buffer.Length)) -gt 0) { $warmed += $n }
        } finally {
            $reader.Dispose()
        }
    }
    $warm = Run-Phase 'warm'
    if ($cold.query_ms.Count -ne 30 -or $warm.query_ms.Count -ne 150) { throw 'Sample count mismatch' }
    [pscustomobject]@{trial=$Trial; engine=$engine; warmed_bytes=$warmed; cold=$cold; warm=$warm} |
        ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $trialDir 'measurement.json')
} catch {
    if (Test-Path -LiteralPath $trialDir) {
        $_.Exception.Message | Set-Content -LiteralPath (Join-Path $trialDir 'reader-error.txt')
    }
    throw
} finally {
    if ($null -ne $done) { [void]$done.Set(); $done.Dispose() }
    if ($null -ne $go) { $go.Dispose() }
}
