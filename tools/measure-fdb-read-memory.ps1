# Read-only requests against a healthy DB. Normal open/recovery may rebuild cache.
# Private is sampled after replies, not a peak. Peak WS is cumulative since launch.
param(
    [Parameter(Mandatory=$true)][string]$DatabasePath,
    [string]$BinaryPath='target/release/siftwake.exe',
    [string]$OutputPath='output/fdb-read-memory.json'
)
$ErrorActionPreference='Stop'
$startInfo=[System.Diagnostics.ProcessStartInfo]::new()
$startInfo.FileName=[System.IO.Path]::GetFullPath($BinaryPath)
$startInfo.ArgumentList.Add([System.IO.Path]::GetFullPath($DatabasePath))
$startInfo.ArgumentList.Add('--stdio')
$startInfo.UseShellExecute=$false
$startInfo.CreateNoWindow=$true
$startInfo.RedirectStandardInput=$true
$startInfo.RedirectStandardOutput=$true
$proc=[System.Diagnostics.Process]::Start($startInfo)
$samples=[System.Collections.Generic.List[object]]::new()
function Request($value) {
    $proc.StandardInput.WriteLine(($value | ConvertTo-Json -Compress -Depth 8))
    $proc.StandardInput.Flush()
    $line=$proc.StandardOutput.ReadLine()
    if (-not $line) { throw 'FDB closed without replying' }
    $reply=$line | ConvertFrom-Json
    if (-not $reply.ok) { throw $reply.error }
    return $reply.result
}
function Sample($phase, $total, $hits) {
    $proc.Refresh()
    $samples.Add([ordered]@{
        phase=$phase; total=$total; hits=$hits;
        working_set_bytes=$proc.WorkingSet64;
        sampled_private_bytes=$proc.PrivateMemorySize64;
        cumulative_peak_working_set_bytes=$proc.PeakWorkingSet64
    })
}
try {
    $stats=Request @{op='stats'}
    Start-Sleep -Milliseconds 1000
    Sample 'idle' $stats.documents $null
    foreach ($name in @('exact','second_page','range','text','last_page')) {
        $query=@{scope='shared';collection='contents';limit=50;offset=0}
        if ($name -eq 'text') { $query.text='teknoloji' }
        else { $query.exact=@{source='synthetic'} }
        if ($name -eq 'second_page') { $query.offset=50 }
        if ($name -eq 'last_page') { $query.offset=999900 }
        if ($name -eq 'range') { $query.ranges=@(@{field='day';min=2;max=5}) }
        $page=Request @{op='search';query=$query}
        Sample $name $page.total $page.hits.Count
    }
    $result=[ordered]@{
        binary=$startInfo.FileName;
        binary_sha256=(Get-FileHash -LiteralPath $startInfo.FileName -Algorithm SHA256).Hash;
        cache='warm OS cache; one request per phase';
        private_is_peak=$false; ws_peak_is_cumulative=$true;
        sequence=$stats.sequence;segments=$stats.segments;samples=$samples
    }
    $result | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 -LiteralPath $OutputPath
    $result | ConvertTo-Json -Depth 8
} finally {
    if (-not $proc.HasExited) {
        $proc.StandardInput.Close()
        $proc.WaitForExit()
    }
    $proc.Dispose()
}
