param([string]$Prefix='docs/benchmarks/reclaim-write-10k-20261017')
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
if(Test-Path -LiteralPath "$Prefix-design.json"){throw 'Preserve existing report'}
$programs=@{control='output/seg05-default-memory-20261017.exe';fdb='output/reclaim-memory-20261017.exe'}
$reader='output/reclaim-reads-20261017.exe'
function Protected-Files {
    @('data/rkyv-corpus-20261009/json.payload-bench','data/benchmark-work/sqlite/sqlite.db','tests/fixtures/v1/checkpoint.fdb','tests/fixtures/v1/checkpoint.ready') | ForEach-Object {
        [PSCustomObject]@{path=$_;bytes=(Get-Item -LiteralPath $_).Length;sha256=(Get-FileHash -LiteralPath $_).Hash}
    }
}
function Live-Files {
    $root='data/benchmark-work/fdb'
    $ready=Get-ChildItem -LiteralPath $root -Filter 'generation-*.ready' -File | Sort-Object Name | Select-Object -Last 1
    $bytes=[IO.File]::ReadAllBytes($ready.FullName)
    if([Text.Encoding]::ASCII.GetString($bytes,0,8) -ne 'FDBMAN03'){throw 'Expected native manifest'}
    $m=[Text.Encoding]::UTF8.GetString($bytes,20,[BitConverter]::ToInt32($bytes,16)) | ConvertFrom-Json
    $names=@($ready.Name,('generation-{0:x16}.fdb' -f [long]$m.active.id))
    foreach($s in $m.runs){
        if($s.kind -ne 'Run'){throw 'Expected native source'}
        $stem='run-{0:x16}' -f [long]$s.id
        $names+=@("$stem.fdb","$stem.idx","$stem.records04.idx")
    }
    @($names | Sort-Object | ForEach-Object {
        $path=Join-Path $root $_
        [PSCustomObject]@{name=$_;bytes=(Get-Item -LiteralPath $path).Length;sha256=(Get-FileHash -LiteralPath $path).Hash}
    })
}
$protected=@(Protected-Files)
$protected | ConvertTo-Json | Set-Content "$Prefix-protected-before.json" -Encoding utf8
[PSCustomObject]@{pairs=2;orders=@('control/fdb','fdb/control');input='same existing first10000;500 records/transaction;20 synced streamed commits; no corpus preparation';cache='OS cache, no eviction or hardware/power isolation';scope='write plus final flush/drain;40 boundary gets inside timing; full10000 payload and4 contains oracle outside timing; OS lifetime process memory; no query performance gate';database='only fixed FDB slot reset for indexing; SQLite/fixture/corpus hashes protected';compatibility='live source/index/MET04/ready hashes must be identical; obsolete files intentionally differ';executables=@(foreach($e in 'control','fdb'){[PSCustomObject]@{variant=$e;path=$programs[$e];sha256=(Get-FileHash -LiteralPath $programs[$e]).Hash}})} | ConvertTo-Json -Depth 5 | Set-Content "$Prefix-design.json" -Encoding utf8
$reference=$null
foreach($pair in 0..1){
    $order=@('control','fdb');if($pair){[Array]::Reverse($order)}
    foreach($variant in $order){
        $stem="$Prefix-$variant-$pair"
        & "$PSScriptRoot/measure-content-memory.ps1" -Prefix ([IO.Path]::GetFileName($stem)) -FdbExecutable $programs[$variant] -FdbFeatures "normal SEG05; $variant retention" -StreamedInput -Engines fdb -IdleRuns 0
        if(-not $?){throw 'Write probe failed'}
        & $programs[$variant] verify data/benchmark-work/fdb 0 10000 > "$stem-verify.jsonl"
        if($LASTEXITCODE -ne 0){throw 'Full payload oracle failed'}
        $live=@(Live-Files)
        $live | ConvertTo-Json | Set-Content "$stem-live.json" -Encoding utf8
        if($null -eq $reference){$reference=ConvertTo-Json $live -Compress}
        elseif($reference -ne (ConvertTo-Json $live -Compress)){throw 'Live source/index/ready bytes differ'}
        Write-Output "Completed write pair $pair $variant; live bytes equal and10000 payloads verified"
    }
}
# The last control has retained files. Open with the new engine to reclaim them
# automatically; compare every live file before/after without any new ingest.
& $reader --matrix 17 --matrix-check > "$Prefix-final-auto-reclaim.jsonl"
if($LASTEXITCODE -ne 0){throw 'Final automatic cleanup/oracle failed'}
$final=@(Live-Files)
if($reference -ne (ConvertTo-Json $final -Compress)){throw 'Final automatic cleanup changed live bytes'}
$final | ConvertTo-Json | Set-Content "$Prefix-final-live.json" -Encoding utf8
Get-ChildItem -LiteralPath data/benchmark-work/fdb -File | Select-Object Name,Length | ConvertTo-Json | Set-Content "$Prefix-final-files.json" -Encoding utf8
$after=@(Protected-Files)
$after | ConvertTo-Json | Set-Content "$Prefix-protected-after.json" -Encoding utf8
if((ConvertTo-Json $protected -Compress) -ne (ConvertTo-Json $after -Compress)){throw 'Protected corpus/SQLite/fixture changed'}
