param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$DatabasePath,
    [Parameter(Mandatory=$true)][string]$OutputPrefix,
    [int]$SampleMilliseconds=20
)
$ErrorActionPreference='Stop'
if (-not (Test-Path -LiteralPath $DatabasePath -PathType Container)) { throw 'An existing healthy benchmark DB is required.' }
if ($SampleMilliseconds -lt 10) { throw 'Sample interval must be >= 10 ms.' }
$exePath=[System.IO.Path]::GetFullPath($Executable)
$dbPath=[System.IO.Path]::GetFullPath($DatabasePath)
$prefix=[System.IO.Path]::GetFullPath($OutputPrefix)
New-Item -ItemType Directory -Force -Path ([System.IO.Path]::GetDirectoryName($prefix)) | Out-Null
$proc=Start-Process -FilePath $exePath -ArgumentList @(('"'+$dbPath+'"')) -WindowStyle Hidden -PassThru -RedirectStandardOutput ($prefix+'.jsonl') -RedirectStandardError ($prefix+'.stderr')
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
    Start-Sleep -Milliseconds $SampleMilliseconds
}
$proc.WaitForExit()
$result=[ordered]@{exit_code=$proc.ExitCode;executable=$exePath;elapsed_s=((Get-Date)-$started).TotalSeconds;peak_working_set_bytes=$peakResident;sampled_peak_private_bytes=$peakPrivate;sample_interval_ms=$SampleMilliseconds;samples=$samples;database=$dbPath;stdout=($prefix+'.jsonl');workload='open and ready queries; no API commits';includes_os_file_cache=$false}
$result | ConvertTo-Json | Set-Content -Encoding utf8 -LiteralPath ($prefix+'.memory.json')
$result | ConvertTo-Json
if ($proc.ExitCode -ne 0) { Get-Content -LiteralPath ($prefix+'.stderr'); throw 'Ready benchmark failed' }
