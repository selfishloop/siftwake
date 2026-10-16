param(
    [string]$Prefix='docs/benchmarks/borrowed-source-10k-20261013',
    [string]$Control='output/borrow-verify-control-20261013.exe',
    [switch]$MeasureStrict,
    [switch]$CheckMatrix,
    [switch]$MeasureMemory
)
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$programs=@{control=$Control;fdb='target/release/examples/measure_content_reads.exe'}
function Invoke-Read($program, $output, [switch]$Warm) {
    $probeArgs=@('--record-reads'); if(-not $Warm){$probeArgs+='--deferred-probe'}
    if (-not $MeasureMemory) {
        & $program @probeArgs > $output
        if ($LASTEXITCODE -ne 0) {throw 'First use failed'}
        return
    }
    $process=Start-Process -FilePath (Resolve-Path -LiteralPath $program).Path -ArgumentList $probeArgs -PassThru -WindowStyle Hidden -RedirectStandardOutput $output -RedirectStandardError "$output.stderr"
    $private=0L; $working=0L; $samples=0
    while (-not $process.WaitForExit(10)) {
        $process.Refresh()
        if (-not $process.HasExited) {
            $private=[Math]::Max($private,$process.PrivateMemorySize64)
            $working=[Math]::Max($working,$process.PeakWorkingSet64)
            $samples++
        }
    }
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) {throw 'First use failed'}
    $scope=if($Warm){'whole process including oracle preparation, full warmup, gets and four warm queries; sampled'}else{'whole process open/get/first-search/repeat; sampled, not engine allocation counters'}
    [PSCustomObject]@{sample_interval_ms=10;samples=$samples;observed_private_peak_bytes=$private;observed_os_peak_working_set_bytes=$working;scope=$scope;short_lived_peaks_may_be_missed=$true} | ConvertTo-Json | Set-Content "$output.memory.json" -Encoding utf8
    $process.Dispose()
}
if (Test-Path -LiteralPath "$prefix-before.json") {throw 'Preserve existing report'}
if (-not (Get-ChildItem -LiteralPath data/benchmark-work/fdb -Filter '*.ready' -File)) {throw 'Existing DB required'}
function Snapshot-Files {
    @(Get-ChildItem -LiteralPath data/benchmark-work/fdb -File | Sort-Object FullName | ForEach-Object {
        [PSCustomObject]@{path=$_.FullName;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}
    })
}
$before=Snapshot-Files
$before | ConvertTo-Json | Set-Content "$prefix-before.json" -Encoding utf8
$programs.GetEnumerator() | ForEach-Object {[PSCustomObject]@{engine=$_.Key;path=$_.Value;sha256=(Get-FileHash -LiteralPath $_.Value).Hash}} | ConvertTo-Json | Set-Content "$prefix-executables.json" -Encoding utf8
foreach ($engine in @('control','fdb')) {
    & $programs[$engine] --record-reads --open-only > "$prefix-$engine-warmup.jsonl"
    if ($LASTEXITCODE -ne 0) {throw 'Warmup failed'}
}
foreach ($n in 0..4) {
    $order=@('control','fdb'); if ($n % 2) {$order=@('fdb','control')}
    foreach ($engine in $order) {
        Invoke-Read $programs[$engine] "$prefix-first-use-$engine-$n.jsonl"
    }
    Write-Output "First use pair $n complete"
}
if ($MeasureStrict) {
    foreach ($n in 0..4) {
        $order=@('control','fdb'); if ($n % 2) {$order=@('fdb','control')}
        foreach ($engine in $order) {
            & $programs[$engine] --open-only > "$prefix-strict-$engine-$n.jsonl"
            if ($LASTEXITCODE -ne 0) {throw 'Strict open failed'}
        }
    }
}
foreach ($n in 0..2) {
    $order=@('control','fdb'); if ($n % 2) {$order=@('fdb','control')}
    foreach ($engine in $order) {
        Invoke-Read $programs[$engine] "$prefix-reads-$engine-$n.jsonl" -Warm
    }
    Write-Output "Warm reads pair $n complete"
}
& $programs.fdb --record-reads --check-integrity > "$prefix-integrity.jsonl"
if ($LASTEXITCODE -ne 0) {throw 'Full integrity failed'}
if ($CheckMatrix) {
    & $programs.fdb --record-reads --matrix 17 --matrix-check > "$prefix-matrix-check.jsonl"
    if ($LASTEXITCODE -ne 0) {throw 'Full matrix correctness failed'}
}
$after=Snapshot-Files
$after | ConvertTo-Json | Set-Content "$prefix-after.json" -Encoding utf8
if ((ConvertTo-Json $before -Compress) -ne (ConvertTo-Json $after -Compress)) {throw 'DB files changed'}
Write-Output 'All existing DB and cache files unchanged; no preparation, rebuild or new corpus.'
