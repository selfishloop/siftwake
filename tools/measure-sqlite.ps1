param([Parameter(Mandatory=$true)][string]$DatabasePath, [Parameter(Mandatory=$true)][string]$PythonPath, [int]$Count=1000000, [string]$OutputPrefix='output/sqlite-scale')
$ErrorActionPreference='Stop'
if (Test-Path -LiteralPath $DatabasePath) { throw 'Use a new directory.' }
$prefix=[System.IO.Path]::GetFullPath($OutputPrefix)
New-Item -ItemType Directory -Force -Path ([System.IO.Path]::GetDirectoryName($prefix)) | Out-Null
$scriptPath=[System.IO.Path]::GetFullPath('tools/measure_sqlite.py')
$dbPath=[System.IO.Path]::GetFullPath($DatabasePath)
$proc=Start-Process -FilePath $PythonPath -ArgumentList @('-X','utf8',('"'+$scriptPath+'"'),('"'+$dbPath+'"'),'--count',$Count) -WindowStyle Hidden -PassThru -RedirectStandardOutput ($prefix+'.jsonl') -RedirectStandardError ($prefix+'.stderr')
$peakResident=0L
$peakPrivate=0L
$samples=0
$started=Get-Date
$measurements=[System.Collections.Generic.List[object]]::new()
while (-not $proc.HasExited) {
    try {
        $proc.Refresh()
        $peakResident=[Math]::Max($peakResident,$proc.PeakWorkingSet64)
        $peakPrivate=[Math]::Max($peakPrivate,$proc.PrivateMemorySize64)
        $measurements.Add([ordered]@{elapsed_s=((Get-Date)-$started).TotalSeconds;working_set=$proc.WorkingSet64;private_bytes=$proc.PrivateMemorySize64})
        $samples++
    } catch { if (-not $proc.HasExited) { throw } }
    Start-Sleep -Milliseconds 200
}
$proc.WaitForExit()
$disk=(Get-ChildItem -LiteralPath $dbPath -File | Measure-Object -Property Length -Sum).Sum
$result=[ordered]@{exit_code=$proc.ExitCode;elapsed_s=((Get-Date)-$started).TotalSeconds;peak_working_set_bytes=$peakResident;sampled_peak_private_bytes=$peakPrivate;sample_interval_ms=200;samples=$samples;total_directory_bytes=$disk;database=$dbPath;stdout=($prefix+'.jsonl')}
$result | ConvertTo-Json | Set-Content -Encoding utf8 -LiteralPath ($prefix+'.memory.json')
$measurements | ConvertTo-Json -Compress | Set-Content -Encoding utf8 -LiteralPath ($prefix+'.samples.json')
$result | ConvertTo-Json
if ($proc.ExitCode -ne 0) { Get-Content -LiteralPath ($prefix+'.stderr'); throw 'Benchmark failed' }
