param(
    [string]$Prefix='docs/benchmarks/window-matrix-10k-20261013',
    [string]$Control='output/window-read-control-20261013.exe',
    [string]$Candidate='target/release/examples/measure_content_reads.exe',
    [ValidateRange(2,12)][int]$Pairs=4,
    [switch]$SkipSqliteReference,
    [switch]$PagesOnly,
    [switch]$StrictOpen,
    # Optional diagnostic isolation; zero preserves the original launch path.
    [ValidateRange(0,2147483647)][long]$AffinityMask=0
)
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
if($Pairs%2){throw 'Use an even pair count to balance first/second process order'}
$programs=@{control=$Control;fdb=$Candidate;sqlite='tools/native_sqlite/target/release/content_reads.exe'}
if(Test-Path -LiteralPath "$prefix-before.json"){throw 'Preserve existing report'}
function Snapshot-Files {
    @(Get-ChildItem -LiteralPath data/benchmark-work/fdb -File | Sort-Object FullName | ForEach-Object {[PSCustomObject]@{path=$_.FullName;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}})
    $sql=Get-Item -LiteralPath data/benchmark-work/sqlite/sqlite.db
    [PSCustomObject]@{path=$sql.FullName;bytes=$sql.Length;sha256=(Get-FileHash -LiteralPath $sql.FullName).Hash}
}
$before=@(Snapshot-Files);$before | ConvertTo-Json | Set-Content "$prefix-before.json" -Encoding utf8
$programs.GetEnumerator() | ForEach-Object {[PSCustomObject]@{engine=$_.Key;path=$_.Value;sha256=(Get-FileHash -LiteralPath $_.Value).Hash}} | ConvertTo-Json | Set-Content "$prefix-executables.json" -Encoding utf8
$orders=@(foreach($n in 0..($Pairs-1)){
    $order=@('control','fdb');if($n%2){$order=@('fdb','control')}
    [PSCustomObject]@{pair=$n;engines=$order}
})
[PSCustomObject]@{pairs=$Pairs;orders=$orders;balanced_first_position=$true;pages_only=[bool]$PagesOnly;strict_open=[bool]$StrictOpen;processor_affinity_mask=$AffinityMask;cache='warm OS cache, no eviction'} | ConvertTo-Json -Depth 5 | Set-Content "$prefix-design.json" -Encoding utf8
foreach($n in 0..($Pairs-1)){
    $order=@('control','fdb');if($n%2){$order=@('fdb','control')}
    foreach($engine in $order){
        $probeArguments=@('--matrix',$n)
        if(-not $StrictOpen){$probeArguments=@('--record-reads')+$probeArguments}
        if($PagesOnly){$probeArguments += '--pages-only'}
        if($AffinityMask -eq 0){
            & $programs[$engine] @probeArguments | Out-File -LiteralPath "$prefix-$engine-$n.jsonl" -Encoding utf8
            if($LASTEXITCODE -ne 0){throw 'Matrix failed'}
        }else{
            $launch=[Diagnostics.Stopwatch]::StartNew()
            $process=Start-Process -FilePath (Resolve-Path -LiteralPath $programs[$engine]).Path -ArgumentList $probeArguments -PassThru -WindowStyle Hidden -RedirectStandardOutput "$prefix-$engine-$n.jsonl" -RedirectStandardError "$prefix-$engine-$n.stderr"
            try{
                $process.ProcessorAffinity=[IntPtr]$AffinityMask
                $appliedMs=$launch.Elapsed.TotalMilliseconds
                $actualMask=$process.ProcessorAffinity.ToInt64()
                if($actualMask -ne $AffinityMask){throw 'Affinity was not applied'}
                $process.WaitForExit()
                $process.Refresh()
                [PSCustomObject]@{pid=$process.Id;exit_code=$process.ExitCode;affinity_mask=$actualMask;affinity_applied_after_ms=$appliedMs;wall_ms=$launch.Elapsed.TotalMilliseconds;process_cpu_ms=$process.TotalProcessorTime.TotalMilliseconds;start_utc=$process.StartTime.ToUniversalTime().ToString('o')} | ConvertTo-Json | Set-Content "$prefix-$engine-$n-process.json" -Encoding utf8
                if($process.ExitCode -ne 0){throw 'Pinned matrix failed'}
            }finally{
                if(-not $process.HasExited){$process.Kill();$process.WaitForExit()}
                $process.Dispose()
            }
        }
    }
    Write-Output "Matrix pair $n complete"
}
# One four-query SQLite reference process, not a repeated SQL performance gate.
if(-not $SkipSqliteReference){
    & $programs.sqlite | Out-File -LiteralPath "$prefix-sqlite-reference.jsonl" -Encoding utf8
    if($LASTEXITCODE -ne 0){throw 'SQLite reference failed'}
}
$after=@(Snapshot-Files);$after | ConvertTo-Json | Set-Content "$prefix-after.json" -Encoding utf8
if((ConvertTo-Json $before -Compress) -ne (ConvertTo-Json $after -Compress)){throw 'DB files changed'}
Write-Output 'FDB/cache and SQLite database hashes unchanged.'
