param(
    [string]$Prefix='docs/benchmarks/default-seg05-10k-20261017',
    [string]$Control='output/compact-cache-memory-20261017.exe',
    [string]$Candidate='output/seg05-default-memory-20261017.exe',
    [string]$ControlReader='output/compact-cache-window-reads-20261017.exe',
    [string]$CandidateReader='output/seg05-default-reads-20261017.exe',
    [ValidateSet(2,4)][int]$Pairs=4,
    [switch]$OwnedInput
)
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
. "$PSScriptRoot/windows-process-memory.ps1"
Initialize-FdbProcessMemoryCounters
if(Test-Path -LiteralPath "$Prefix-design.json"){throw 'Preserve existing report'}
$programs=@{control=$Control;fdb=$Candidate}
$readers=@{control=$ControlReader;fdb=$CandidateReader}
function Protected-Files {
    @('data/rkyv-corpus-20261009/json.payload-bench',
      'data/benchmark-work/sqlite/sqlite.db',
      'tests/fixtures/v1/checkpoint.fdb',
      'tests/fixtures/v1/checkpoint.ready') | ForEach-Object {
        $file=Get-Item -LiteralPath $_
        [PSCustomObject]@{path=$_;bytes=$file.Length;sha256=(Get-FileHash -LiteralPath $_).Hash}
    }
}
function Source-Snapshot {
    @(Get-ChildItem -LiteralPath data/benchmark-work/fdb -File |
      Where-Object {$_.Extension -in @('.fdb','.ready')} | Sort-Object Name |
      ForEach-Object {[PSCustomObject]@{name=$_.Name;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}})
}
function Live-Indexes {
    $root='data/benchmark-work/fdb'
    $ready=Get-ChildItem -LiteralPath $root -Filter 'generation-*.ready' -File | Sort-Object Name | Select-Object -Last 1
    $bytes=[IO.File]::ReadAllBytes($ready.FullName)
    if([Text.Encoding]::ASCII.GetString($bytes,0,8) -ne 'FDBMAN03'){throw 'Expected native manifest'}
    $manifest=[Text.Encoding]::UTF8.GetString($bytes,20,[BitConverter]::ToInt32($bytes,16)) | ConvertFrom-Json
    foreach($run in $manifest.runs){
        if($run.kind -ne 'Run'){throw 'Expected native Run'}
        $name='run-{0:x16}' -f [long]$run.id
        $path=Join-Path $root "$name.idx"
        $file=[IO.File]::OpenRead([IO.Path]::GetFullPath($path))
        try {
            $header=[byte[]]::new(8);$null=$file.Read($header,0,8)
            $footer=[byte[]]::new(32);$null=$file.Seek(-32,[IO.SeekOrigin]::End);$null=$file.Read($footer,0,32)
            $directory=[BitConverter]::ToInt64($footer,8)
            $flag=$null
            if([Text.Encoding]::ASCII.GetString($header) -eq 'FDBSEG05'){
                $null=$file.Seek($directory+8,[IO.SeekOrigin]::Begin);$flag=$file.ReadByte()
            }
            [PSCustomObject]@{name="$name.idx";bytes=$file.Length;magic=[Text.Encoding]::ASCII.GetString($header);directory_bytes=[BitConverter]::ToInt64($footer,16);references_flag=$flag;metadata_bytes=(Get-Item -LiteralPath (Join-Path $root "$name.records04.idx")).Length}
        } finally {$file.Dispose()}
    }
}
function Read-Probe([string]$Engine,[string]$Stem,[string[]]$Arguments,[switch]$Memory) {
    $process=Start-Process -FilePath (Resolve-Path -LiteralPath $readers[$Engine]).Path -ArgumentList $Arguments -WindowStyle Hidden -PassThru -RedirectStandardOutput "$Stem.jsonl" -RedirectStandardError "$Stem.stderr"
    $handle=$process.Handle
    try {
        $process.ProcessorAffinity=[IntPtr]4
        $process.WaitForExit()
        $process.Refresh()
        if($process.ExitCode -ne 0){throw "Read probe failed: $Stem"}
        if($Memory){
            [PSCustomObject]@{exit_code=$process.ExitCode;os_lifetime_peaks=(Get-FdbProcessMemoryPeaks $handle)} | ConvertTo-Json -Depth 5 | Set-Content "$Stem.memory.json" -Encoding utf8
        }
        $first=Get-Content -LiteralPath "$Stem.jsonl" -First 1 | ConvertFrom-Json
        if($first.phase -ne 'opened' -or $first.pending_source_checks -ne 0){throw 'Expected strict validation/pending0'}
    } finally {
        if(-not $process.HasExited){$process.Kill();$process.WaitForExit()}
        $process.Dispose()
    }
}
$before=@(Protected-Files)
$before | ConvertTo-Json | Set-Content "$Prefix-protected-before.json" -Encoding utf8
[PSCustomObject]@{
    pairs=$Pairs
    orders=@(foreach($n in 0..($Pairs-1)){if($n -in @(1,2)){'fdb/control'}else{'control/fdb'}})
    input='same first10000 existing CRC-checked corpus records;500/transaction;20 transactions; synced WAL'
    input_api=$(if($OwnedInput){'commit(Vec<Mutation>)'}else{'commit_streamed(iterator)'})
    cache='warm OS cache; fresh processes; no eviction; untimed strict first-use before timed reads'
    validation='full source/frame/index CRC, schema, versions, directory/page checks; pending0'
    writes='only owned fixed FDB slot reset for ingest; no corpus generation, DB copies or SQLite writes'
    reads='same38-case full payload/version oracle; common, rare, two-character, AND, negative and page queries; separate oracle-free24search/771get memory'
    affinity='read probes mask4; ingest not pinned; no hardware or power isolation'
    diagnostics=$false
    executables=@(foreach($e in 'control','fdb'){
        [PSCustomObject]@{variant=$e;writer=$programs[$e];writer_sha256=(Get-FileHash -LiteralPath $programs[$e]).Hash;reader=$readers[$e];reader_sha256=(Get-FileHash -LiteralPath $readers[$e]).Hash}
    })
} | ConvertTo-Json -Depth 6 | Set-Content "$Prefix-design.json" -Encoding utf8
$reference=$null
foreach($pair in 0..($Pairs-1)){
    $order=@('control','fdb');if($pair -in @(1,2)){[Array]::Reverse($order)}
    foreach($engine in $order){
        $stem="$Prefix-$engine-$pair"
        & "$PSScriptRoot/measure-content-memory.ps1" -Prefix ([IO.Path]::GetFileName($stem)) -FdbExecutable $programs[$engine] -FdbFeatures "normal bounded-engine; $engine writer" -StreamedInput:(-not $OwnedInput) -Engines fdb -IdleRuns 0
        if(-not $?){throw 'Write measurement failed'}
        & $programs[$engine] verify data/benchmark-work/fdb 0 10000 > "$stem-verify.jsonl"
        if($LASTEXITCODE -ne 0){throw 'Full10000 document/contains verification failed'}
        $snapshot=@(Source-Snapshot)
        $snapshot | ConvertTo-Json | Set-Content "$stem-sources.json" -Encoding utf8
        if($null -eq $reference){$reference=ConvertTo-Json $snapshot -Compress}
        elseif($reference -ne (ConvertTo-Json $snapshot -Compress)){throw 'Source/ready bytes differ between writers'}
        # No explicit repack: these probes read the normal writer's output.
        Read-Probe $engine "$stem-warmup" @('--deferred-probe')
        Read-Probe $engine "$stem-open" @('--deferred-probe')
        Read-Probe $engine "$stem-query-memory" @('--memory-probe') -Memory
        Read-Probe $engine "$stem-matrix" @('--matrix',"$pair")
        $endSnapshot=@(Source-Snapshot)
        if((ConvertTo-Json $snapshot -Compress) -ne (ConvertTo-Json $endSnapshot -Compress)){throw 'Read changed source/ready'}
        @(Live-Indexes) | ConvertTo-Json | Set-Content "$stem-live-indexes.json" -Encoding utf8
        Write-Output "Completed default writer pair $pair $engine; full payload/source checks passed"
    }
}
$after=@(Protected-Files)
$after | ConvertTo-Json | Set-Content "$Prefix-protected-after.json" -Encoding utf8
if((ConvertTo-Json $before -Compress) -ne (ConvertTo-Json $after -Compress)){throw 'Corpus/SQLite/fixture changed'}
