param([string]$Prefix='ram-stream-10k-20261013',
      [string]$Control='output/ram-write-control-20261013.exe',
      [string]$Candidate='output/ram-stream-candidate-20261013.exe',
      [string]$ControlFeatures='experimental-merge-postings experimental-delta-bitmaps',
      [string]$CandidateFeatures='experimental-merge-postings experimental-delta-bitmaps',
      [switch]$ControlStreamedInput,
      [switch]$CandidateStreamedInput)
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$programs=@{control=$Control;candidate=$Candidate}
if(Get-ChildItem -LiteralPath docs/benchmarks -Filter "$Prefix-*.jsonl"){throw 'Preserve existing logs'}
$programs.GetEnumerator() | ForEach-Object {[PSCustomObject]@{variant=$_.Key;path=$_.Value;sha256=(Get-FileHash -LiteralPath $_.Value).Hash}} | ConvertTo-Json | Set-Content "docs/benchmarks/$Prefix-executables.json" -Encoding utf8
foreach($run in 0..2){
    $order=@('control','candidate');if($run%2){[Array]::Reverse($order)}
    foreach($variant in $order){
        $features=if($variant -eq 'candidate'){$CandidateFeatures}else{$ControlFeatures}
        $streamed=if($variant -eq 'candidate'){$CandidateStreamedInput}else{$ControlStreamedInput}
        & ./tools/measure-content-memory.ps1 -Prefix "$Prefix-$variant-$run" -FdbExecutable $programs[$variant] -FdbFeatures $features -Engines fdb -IdleRuns 1 -StreamedInput:$streamed
        & $programs[$variant] verify data/benchmark-work/fdb 0 10000 | Out-File "docs/benchmarks/$Prefix-$variant-$run-verify.jsonl" -Encoding utf8
        if($LASTEXITCODE -ne 0){throw 'Full document/query verification failed'}
        if($variant -eq 'candidate'){
            # The previous executable must read the current codec unchanged.
            & $programs.control verify data/benchmark-work/fdb 0 10000 | Out-File "docs/benchmarks/$Prefix-$variant-$run-old-reader.jsonl" -Encoding utf8
            if($LASTEXITCODE -ne 0){throw 'Previous reader compatibility failed'}
        }
    }
    Write-Output "RAM/write pair $run verified"
}
