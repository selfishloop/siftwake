param([Parameter(Mandatory=$true)][string]$DatabasePath, [int]$Count=1000000, [int]$TextBytes=256, [string]$OutputPrefix='output/scale')
$ErrorActionPreference='Stop'
if (Test-Path -LiteralPath $DatabasePath) { throw 'Use a new database directory.' }
$prefix=[System.IO.Path]::GetFullPath($OutputPrefix)
New-Item -ItemType Directory -Force -Path ([System.IO.Path]::GetDirectoryName($prefix)) | Out-Null
$exe=[System.IO.Path]::GetFullPath('target/release/examples/measure_scale.exe')
$dbPath=[System.IO.Path]::GetFullPath($DatabasePath)
$proc=Start-Process -FilePath $exe -ArgumentList @(('"'+$dbPath+'"'),$Count,$TextBytes) -WindowStyle Hidden -PassThru -RedirectStandardOutput ($prefix+'.jsonl') -RedirectStandardError ($prefix+'.stderr')
$peakResident=0L
$peakPrivate=0L
$samples=0
$started=Get-Date
while (-not $proc.HasExited) {
    try {
        $proc.Refresh()
        $peakResident=[Math]::Max($peakResident,$proc.PeakWorkingSet64)
        $peakPrivate=[Math]::Max($peakPrivate,$proc.PrivateMemorySize64)
        $samples++
    } catch { if (-not $proc.HasExited) { throw } }
    Start-Sleep -Milliseconds 200
}
$proc.WaitForExit()
$disk=(Get-ChildItem -LiteralPath $dbPath -File | Measure-Object -Property Length -Sum).Sum
$result=[ordered]@{exit_code=$proc.ExitCode;elapsed_s=((Get-Date)-$started).TotalSeconds;peak_working_set_bytes=$peakResident;sampled_peak_private_bytes=$peakPrivate;sample_interval_ms=200;samples=$samples;total_directory_bytes=$disk;database=$dbPath;stdout=($prefix+'.jsonl')}
$result | ConvertTo-Json | Set-Content -Encoding utf8 -LiteralPath ($prefix+'.memory.json')
$result | ConvertTo-Json
if ($proc.ExitCode -ne 0) { Get-Content -LiteralPath ($prefix+'.stderr'); throw 'Benchmark failed' }
