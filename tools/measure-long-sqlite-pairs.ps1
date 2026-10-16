param([string]$FdbDatabase='data/engine-fold-v2-20261011',[string]$SqliteDatabase='data/sqlite-passages-20261011',[string]$Prefix='long-sqlite-20261011')
$ErrorActionPreference='Stop'
$longFdb=[IO.Path]::GetFullPath($FdbDatabase)
$longSqlite=[IO.Path]::GetFullPath($SqliteDatabase)
for ($longRun=0;$longRun -lt 3;$longRun++) {
    $longEngines=@('fdb','sqlite')
    if ($longRun % 2 -eq 1) { [Array]::Reverse($longEngines) }
    foreach ($longEngine in $longEngines) {
        if ($longEngine -eq 'fdb') {
            $longExe=[IO.Path]::GetFullPath('output/fdb-engine-fold-candidate-20261011.exe')
            $longArgs=@(('"'+$longFdb+'"'))
        } else {
            $longExe=[IO.Path]::GetFullPath('output/sqlite-passages-20261011.exe')
            $longArgs=@('query',('"'+$longSqlite+'"'),('"'+$longFdb+'"'))
        }
        $longLog=[IO.Path]::GetFullPath("docs/benchmarks/$Prefix-$longEngine-$longRun")
        if(Test-Path -LiteralPath ($longLog+'.jsonl')) {throw 'Use a new prefix'}
        $longProcess=Start-Process -FilePath $longExe -ArgumentList $longArgs -WindowStyle Hidden -PassThru -RedirectStandardOutput ($longLog+'.jsonl') -RedirectStandardError ($longLog+'.stderr')
        $longPeak=0L;$longPrivate=0L;$longSamples=0;$longStart=Get-Date
        while(-not $longProcess.HasExited) {
            try {$longProcess.Refresh();$longPeak=[Math]::Max($longPeak,$longProcess.PeakWorkingSet64);$longPrivate=[Math]::Max($longPrivate,$longProcess.PrivateMemorySize64);$longSamples++}
            catch {if(-not $longProcess.HasExited){throw}}
            Start-Sleep -Milliseconds 20
        }
        $longProcess.WaitForExit()
        $longMemory=[ordered]@{engine=$longEngine;run=$longRun;exit_code=$longProcess.ExitCode;executable=$longExe;fdb_database=$longFdb;sqlite_database=$longSqlite;peak_working_set_bytes=$longPeak;sampled_peak_private_bytes=$longPrivate;sample_interval_ms=20;samples=$longSamples;elapsed_s=((Get-Date)-$longStart).TotalSeconds;cpu_ms=$longProcess.TotalProcessorTime.TotalMilliseconds;window='whole native process: open + corpus plan/books/oracle + queries; no integrity command';includes_os_file_cache=$false}
        $longMemory | ConvertTo-Json | Set-Content -Encoding utf8 -LiteralPath ($longLog+'.memory.json')
        $longMemory | ConvertTo-Json
        if($longProcess.ExitCode -ne 0){Get-Content -LiteralPath ($longLog+'.stderr');throw 'Long corpus comparison failed'}
    }
}
