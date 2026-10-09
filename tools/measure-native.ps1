param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$DatabasePath,
    [int]$Count=1000000,
    [int]$TextBytes=256,
    [string]$OutputPrefix='output/native',
    [int]$SampleMilliseconds=20,
    [Nullable[int]]$DeltaKeys,
    [ValidateSet('on','off')][string]$WalSync
)
$ErrorActionPreference='Stop'
if (Test-Path -LiteralPath $DatabasePath) { throw 'Use a new database directory.' }
if ($SampleMilliseconds -lt 10) { throw 'Sample interval must be >= 10 ms.' }
$exePath=[System.IO.Path]::GetFullPath($Executable)
$dbPath=[System.IO.Path]::GetFullPath($DatabasePath)
$prefix=[System.IO.Path]::GetFullPath($OutputPrefix)
New-Item -ItemType Directory -Force -Path ([System.IO.Path]::GetDirectoryName($prefix)) | Out-Null
$benchmarkArgs = @(('"'+$dbPath+'"'),$Count,$TextBytes)
if ($null -ne $DeltaKeys) { if ($DeltaKeys -lt 1) { throw 'Delta keys must be positive.' }; $benchmarkArgs += $DeltaKeys }
if ($WalSync) { if ($null -eq $DeltaKeys) { $benchmarkArgs += 10000 }; $benchmarkArgs += $WalSync }
$proc=Start-Process -FilePath $exePath -ArgumentList $benchmarkArgs -WindowStyle Hidden -PassThru -RedirectStandardOutput ($prefix+'.jsonl') -RedirectStandardError ($prefix+'.stderr')
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
$totalCpuMs=$null
$userCpuMs=$null
try {
    $totalCpuMs=$proc.TotalProcessorTime.TotalMilliseconds
    $userCpuMs=$proc.UserProcessorTime.TotalMilliseconds
} catch { }
$disk=(Get-ChildItem -LiteralPath $dbPath -File | Measure-Object -Property Length -Sum).Sum
$result=[ordered]@{exit_code=$proc.ExitCode;delta_keys_argument=$DeltaKeys;executable=$exePath;elapsed_s=((Get-Date)-$started).TotalSeconds;total_cpu_ms=$totalCpuMs;user_cpu_ms=$userCpuMs;peak_working_set_bytes=$peakResident;sampled_peak_private_bytes=$peakPrivate;sample_interval_ms=$SampleMilliseconds;samples=$samples;total_directory_bytes=$disk;database=$dbPath;stdout=($prefix+'.jsonl');includes_os_file_cache=$false}
$result | ConvertTo-Json | Set-Content -Encoding utf8 -LiteralPath ($prefix+'.memory.json')
$result | ConvertTo-Json
if ($proc.ExitCode -ne 0) { Get-Content -LiteralPath ($prefix+'.stderr'); throw 'Native benchmark failed' }
