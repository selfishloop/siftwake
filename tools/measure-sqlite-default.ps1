param(
    [ValidateSet('write','read','open-memory')][string]$Phase='read',
    [string]$Prefix='docs/benchmarks/sqlite-default-10k-20261016',
    [string]$FdbMemory='target/release/examples/measure_content_memory.exe',
    [string]$FdbReads='target/release/examples/measure_content_reads.exe',
    [string]$SqliteMemory='tools/native_sqlite/target/release/content_memory.exe',
    [string]$SqliteReads='tools/native_sqlite/target/release/content_reads.exe'
)
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
. "$PSScriptRoot/windows-process-memory.ps1"
Initialize-FdbProcessMemoryCounters
if(Test-Path -LiteralPath "$Prefix-$Phase-design.json"){throw 'Preserve existing report; use a new log prefix'}
if(-not (Test-Path -LiteralPath 'data/rkyv-corpus-20261009/json.payload-bench' -PathType Leaf)){throw 'Existing corpus required'}
$programs=@{fdb_memory=$FdbMemory;fdb_reads=$FdbReads;sqlite_memory=$SqliteMemory;sqlite_reads=$SqliteReads}
$programs.GetEnumerator() | ForEach-Object {
    [PSCustomObject]@{role=$_.Key;path=$_.Value;sha256=(Get-FileHash -LiteralPath $_.Value).Hash}
} | ConvertTo-Json | Set-Content -LiteralPath "$Prefix-$Phase-executables.json" -Encoding utf8
[PSCustomObject]@{
    phase=$Phase;git_head=(& git rev-parse HEAD);fdb_features='normal default bounded-engine; no diagnostics'
    corpus='same first10000 existing records;264842858 body bytes;2057-51197 bytes/passage'
    cache='warm OS cache; no eviction, power or hardware isolation; sequential processes'
    write='4 balanced streamed/SQLite pairs;2 balanced owned/SQLite pairs;500/transaction;sync WAL'
    search='same38 cases and256 gets;4 balanced pairs;20 primary/5 page samples/process;full payload/version oracle outside timing'
    opening='4 balanced default strict/SQLite pairs;record-read first-use also explicit;full source integrity only on FDB strict open'
    memory='2 balanced pairs;3 rounds/24 queries/771 gets without corpus/oracle resident;OS lifetime peaks include opening'
    sqlite='bundled FTS5 trigram, FULL WAL,cache2MiB,mmap0,tempFILE;not all FDB field/link/guard functionality'
    storage='only fixed owned data/benchmark-work/fdb and sqlite slots;no corpus generation or DB copies'
} | ConvertTo-Json | Set-Content -LiteralPath "$Prefix-$Phase-design.json" -Encoding utf8
function Snapshot-ComparisonFiles {
    @(Get-ChildItem -LiteralPath data/benchmark-work/fdb,data/benchmark-work/sqlite -File |
        Where-Object {$_.Extension -in @('.fdb','.idx','.ready','.rmp','.svfy') -or $_.Name -eq 'sqlite.db'} |
        Sort-Object FullName | ForEach-Object {
            [PSCustomObject]@{path=$_.FullName;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}
        })
}
function Invoke-ReadProbe([string]$Program,[string[]]$ProbeArgs,[string]$Stem,[switch]$Memory) {
    $log="$Stem.jsonl"
    if(Test-Path -LiteralPath $log){throw "Preserve existing log: $log"}
    if(-not $Memory){
        & $Program @ProbeArgs > $log
        if($LASTEXITCODE -ne 0){throw "Read probe failed: $Stem"}
        return
    }
    $proc=Start-Process -FilePath ([IO.Path]::GetFullPath($Program)) -ArgumentList $ProbeArgs -WindowStyle Hidden -PassThru -RedirectStandardOutput ([IO.Path]::GetFullPath($log)) -RedirectStandardError ([IO.Path]::GetFullPath("$Stem.stderr"))
    $handle=$proc.Handle
    $private=0L;$working=0L;$samples=0
    try {
        while(-not $proc.HasExited){
            try {
                $proc.Refresh()
                $private=[Math]::Max($private,$proc.PrivateMemorySize64)
                $working=[Math]::Max($working,$proc.WorkingSet64)
                $samples++
            } catch {if(-not $proc.HasExited){throw}}
            Start-Sleep -Milliseconds 20
        }
        $proc.WaitForExit()
        [PSCustomObject]@{exit_code=$proc.ExitCode;cpu_ms=$proc.TotalProcessorTime.TotalMilliseconds;os_lifetime_peaks=(Get-FdbProcessMemoryPeaks $handle);sampled_peak_private_bytes=$private;sampled_peak_working_set_bytes=$working;samples=$samples;sample_interval_ms=20;scope='open plus query/get result buffers;no corpus/oracle;not total system RAM'} |
            ConvertTo-Json -Depth 5 | Set-Content -LiteralPath "$Stem.memory.json" -Encoding utf8
        if($proc.ExitCode -ne 0){throw "Memory probe failed: $Stem"}
    } finally {$proc.Dispose()}
}
if($Phase -eq 'write'){
    foreach($api in @('owned','streamed')){
        $pairs=if($api -eq 'streamed'){4}else{2}
        foreach($pair in 0..($pairs-1)){
            $order=@('fdb','sqlite');if($pair%2){[Array]::Reverse($order)}
            foreach($engine in $order){
                $run="$Prefix-$api-$pair"
                & "$PSScriptRoot/measure-content-memory.ps1" -Prefix ([IO.Path]::GetFileName($run)) -FdbExecutable $FdbMemory -SqliteExecutable $SqliteMemory -FdbFeatures 'default bounded-engine' -StreamedInput:($api -eq 'streamed') -Engines $engine -IdleRuns 0
                if(-not $?){throw 'Ingest/memory probe failed'}
                $program=if($engine -eq 'fdb'){$FdbMemory}else{$SqliteMemory}
                & $program verify "data/benchmark-work/$engine" 0 10000 > "$run-$engine-verify.jsonl"
                if($LASTEXITCODE -ne 0){throw 'Full corpus verification failed'}
                Get-ChildItem -LiteralPath "data/benchmark-work/$engine" -File |
                    Where-Object {$_.Extension -in @('.fdb','.idx','.ready') -or $_.Name -eq 'sqlite.db'} |
                    Sort-Object Name | ForEach-Object {
                        [PSCustomObject]@{name=$_.Name;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}
                    } | ConvertTo-Json | Set-Content -LiteralPath "$run-$engine-files.json" -Encoding utf8
            }
            Write-Output "$api write/maintenance pair $pair completed and verified"
        }
    }
    # Read tests reuse the streamed layout left by the last pair; no DB copies.
    return
}
if($Phase -eq 'read'){
    # Prepare derived caches once, outside all measurements, in the same slot.
    & $FdbReads --prepare-record-reads > "$Prefix-prepare-record-reads.jsonl"
    if($LASTEXITCODE -ne 0){throw 'Derived cache preparation failed'}
}
$before=@(Snapshot-ComparisonFiles)
$before | ConvertTo-Json | Set-Content -LiteralPath "$Prefix-$Phase-before.json" -Encoding utf8
if($Phase -eq 'read'){
    foreach($pair in 0..3){
        $order=@('fdb','sqlite');if($pair%2){[Array]::Reverse($order)}
        foreach($engine in $order){
            $program=if($engine -eq 'fdb'){$FdbReads}else{$SqliteReads}
            Invoke-ReadProbe $program @('--matrix',"$pair") "$Prefix-matrix-$engine-$pair"
            Write-Output "38-case read matrix $engine pair $pair completed and verified"
        }
    }
} else {
    foreach($pair in 0..3){
        $order=@('fdb','sqlite');if($pair%2){[Array]::Reverse($order)}
        foreach($engine in $order){
            $program=if($engine -eq 'fdb'){$FdbReads}else{$SqliteReads}
            $probeArgs=if($engine -eq 'fdb'){@('--deferred-probe')}else{@('--first-use-probe')}
            Invoke-ReadProbe $program $probeArgs "$Prefix-open-$engine-$pair"
        }
        Invoke-ReadProbe $FdbReads @('--record-reads','--deferred-probe') "$Prefix-open-record-fdb-$pair"
    }
    foreach($pair in 0..1){
        $order=@('fdb','sqlite');if($pair%2){[Array]::Reverse($order)}
        foreach($engine in $order){
            $program=if($engine -eq 'fdb'){$FdbReads}else{$SqliteReads}
            Invoke-ReadProbe $program @('--memory-probe') "$Prefix-query-memory-$engine-$pair" -Memory
        }
    }
    & "$PSScriptRoot/measure-content-memory.ps1" -Prefix ([IO.Path]::GetFileName($Prefix)) -FdbExecutable $FdbMemory -SqliteExecutable $SqliteMemory -FdbFeatures 'default bounded-engine' -Mode idle -IdleRuns 2
    if(-not $?){throw 'Idle memory probe failed'}
}
$after=@(Snapshot-ComparisonFiles)
$after | ConvertTo-Json | Set-Content -LiteralPath "$Prefix-$Phase-after.json" -Encoding utf8
if((ConvertTo-Json $before -Compress) -ne (ConvertTo-Json $after -Compress)){throw 'Read-only source/index/cache hashes changed'}
Write-Output "$Phase complete; source/index/cache hashes unchanged"
