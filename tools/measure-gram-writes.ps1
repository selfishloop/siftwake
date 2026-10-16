param(
    [string]$Prefix='docs/benchmarks/gram-key-write-10k-20261016',
    [string]$Control='output/gram-key-memory-control-20261016.exe',
    [string]$Candidate='output/gram-key-memory-candidate-20261016.exe',
    [string]$Sqlite='tools/native_sqlite/target/release/content_memory.exe',
    [string]$ControlFeatures='experimental-builder-budget',
    [string]$CandidateFeatures='experimental-builder-budget',
    [ValidateSet(2,4)][int]$Pairs=4,
    [ValidateRange(0,3)][int]$FirstPair=0,
    [switch]$OwnedInput,
    [switch]$SkipSqliteReference
)
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
if(Test-Path -LiteralPath "$Prefix-design.json"){throw 'Preserve existing report'}
if($FirstPair -ge $Pairs){throw 'FirstPair must be below Pairs'}
$programs=@{control=$Control;fdb=$Candidate;sqlite=$Sqlite}
$featureLabels=@{control=$ControlFeatures;fdb=$CandidateFeatures}
$programs.GetEnumerator()|ForEach-Object {[PSCustomObject]@{engine=$_.Key;path=$_.Value;sha256=(Get-FileHash -LiteralPath $_.Value).Hash}}|ConvertTo-Json|Set-Content -LiteralPath "$Prefix-executables.json" -Encoding utf8
$referencePairs=@(@(0,3)|Where-Object {$_ -ge $FirstPair -and $_ -lt $Pairs})
if($SkipSqliteReference){$referencePairs=@()}
[PSCustomObject]@{
    fdb_pairs=$Pairs-$FirstPair
    first_pair=$FirstPair
    balanced_first_position=(($Pairs-$FirstPair)%2 -eq 0)
    orders=@(foreach($n in $FirstPair..($Pairs-1)){if($n%2){'fdb/control'}else{'control/fdb'}})
    sqlite_reference_processes=$referencePairs.Count
    sqlite_references_after_pairs=$referencePairs
    sqlite_is_not_paired_performance_gate=$true
    input='same first10000 existing records;500 records/transaction;20 transactions; synced WAL'
    input_api=$(if($OwnedInput){'commit(Vec<Mutation>); caller collects batch500'}else{'commit_streamed(iterator); caller holds one input record'})
    cache='OS cache, no eviction; no hardware/power isolation'
    query='40 boundary full-document gets inside ingest; independent full verify outside timing'
    databases='only fixed data/benchmark-work/fdb and sqlite slots; reset owned slot per ingest'
    normal_build=$featureLabels
    diagnostic_timing_and_allocation=$false
}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath "$Prefix-design.json" -Encoding utf8
function Snapshot-Core {
    @(Get-ChildItem -LiteralPath data/benchmark-work/fdb -File |
      Where-Object {$_.Extension -in @('.fdb','.idx','.ready')} |
      Sort-Object Name | ForEach-Object {[PSCustomObject]@{name=$_.Name;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}})
}
$reference=$null
foreach($pair in $FirstPair..($Pairs-1)){
    $order=@('control','fdb');if($pair%2){$order=@('fdb','control')}
    foreach($engine in $order){
        $runPrefix="$Prefix-$engine-$pair"
        $leaf=[IO.Path]::GetFileName($runPrefix)
        & "$PSScriptRoot/measure-content-memory.ps1" -Prefix $leaf -FdbExecutable $programs[$engine] -FdbFeatures $featureLabels[$engine] -StreamedInput:(-not $OwnedInput) -Engines fdb -IdleRuns 0
        if(-not $?){throw 'Measurement failed'}
        & $programs[$engine] verify data/benchmark-work/fdb 0 10000 > "$runPrefix-verify.jsonl"
        if($LASTEXITCODE -ne 0){throw 'Full corpus/contains verification failed'}
        $snapshot=Snapshot-Core
        $snapshot|ConvertTo-Json|Set-Content -LiteralPath "$runPrefix-files.json" -Encoding utf8
        if($null -eq $reference){$reference=ConvertTo-Json $snapshot -Compress}
        elseif($reference -ne (ConvertTo-Json $snapshot -Compress)){throw 'Persistent source/index bytes differ from control'}
    }
    if(-not $SkipSqliteReference -and $pair -in @(0,3)){
        $runPrefix="$Prefix-sqlite-$pair"
        & "$PSScriptRoot/measure-content-memory.ps1" -Prefix ([IO.Path]::GetFileName($runPrefix)) -SqliteExecutable $programs.sqlite -Engines sqlite -IdleRuns 0
        if(-not $?){throw 'SQLite reference failed'}
        & $programs.sqlite verify data/benchmark-work/sqlite 0 10000 > "$runPrefix-verify.jsonl"
        if($LASTEXITCODE -ne 0){throw 'SQLite full verification failed'}
    }
    Write-Output "Write pair $pair complete; source/index bytes equal control"
}
