param([int]$DriverId,[string]$LogPath,[string]$StopPath,[string]$LifecyclePath='')
$ErrorActionPreference='Stop'
$stream=[IO.StreamWriter]::new($LogPath,$false,[Text.UTF8Encoding]::new($false));$stream.AutoFlush=$true
function Emit($value){$value.utc=[DateTime]::UtcNow.ToString('o');$stream.WriteLine(($value|ConvertTo-Json -Compress -Depth 6))}
$tracked=[Collections.Generic.HashSet[int]]::new();[void]$tracked.Add($DriverId)
$ancestors=[Collections.Generic.HashSet[int]]::new()
$sources=@();$begin=[DateTime]::UtcNow
$handles=@{};$seen=[Collections.Generic.HashSet[int]]::new()
function WatchHandle([int]$targetId) {
 if(-not $seen.Add($targetId)){return}
 try {$target=[Diagnostics.Process]::GetProcessById($targetId);[void]$target.Handle;$handles[$targetId]=$target;Emit @{kind='handle_watch';pid=$targetId;ok=$true;name=$target.ProcessName}}
 catch {Emit @{kind='handle_watch';pid=$targetId;ok=$false;error=$_.Exception.Message}}
}
function PollHandles {
 if($LifecyclePath -and (Test-Path -LiteralPath $LifecyclePath)) {
  $readStream=[IO.FileStream]::new($LifecyclePath,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::ReadWrite)
  $reader=[IO.StreamReader]::new($readStream)
  try {$lines=$reader.ReadToEnd() -split "`n"} finally {$reader.Dispose()}
  foreach($line in $lines) {
   try {$row=$line|ConvertFrom-Json;if($row.kind -eq 'worker_start'){WatchHandle ([int]$row.pid)}}catch {}
  }
 }
 foreach($targetId in @($handles.Keys)) {
  $target=$handles[$targetId]
  if($target.HasExited){Emit @{kind='handle_exit';pid=$targetId;exit_code=$target.ExitCode};$target.Dispose();$handles.Remove($targetId)}
 }
}
try {
 Emit @{kind='monitor_start';pid=$PID;driver=$DriverId;session=(Get-Process -Id $PID).SessionId}
 $ancestorId=$DriverId
 for($depth=0;$depth -lt 10 -and $ancestorId -gt 0;$depth++) {
  $proc=Get-CimInstance Win32_Process -Filter "ProcessId=$ancestorId" -ErrorAction SilentlyContinue
  if($null -eq $proc){break}
  Emit @{kind='ancestry';pid=$proc.ProcessId;parent=$proc.ParentProcessId;name=$proc.Name;session=$proc.SessionId}
  [void]$ancestors.Add([int]$proc.ProcessId);$ancestorId=[int]$proc.ParentProcessId
  WatchHandle ([int]$proc.ProcessId)
 }
 foreach($eventClass in @('Win32_ProcessStartTrace','Win32_ProcessStopTrace')) {
  try {$id="write50-$eventClass-$PID";Register-CimIndicationEvent -Query "SELECT * FROM $eventClass" -SourceIdentifier $id|Out-Null;$sources+=$id;Emit @{kind='subscription';class=$eventClass;ok=$true}}
  catch {Emit @{kind='subscription';class=$eventClass;ok=$false;error=$_.Exception.Message}}
 }
 $tick=0
 while(-not(Test-Path -LiteralPath $StopPath)) {
  Wait-Event -Timeout 1|Out-Null
  PollHandles
  foreach($eventItem in @(Get-Event)) {
   $e=$eventItem.SourceEventArgs.NewEvent
   if($null -ne $e) {
    $childId=[int]$e.ProcessID;$parentId=[int]$e.ParentProcessID
    if($tracked.Contains($childId) -or $tracked.Contains($parentId) -or $ancestors.Contains($childId)) {
     if(-not $ancestors.Contains($childId)){[void]$tracked.Add($childId)}
     Emit @{kind='process_event';class=$e.CimClass.CimClassName;pid=$childId;parent=$parentId;name=[string]$e.ProcessName;exit_status=$e.ExitStatus;session=$e.SessionID}
    }
   }
   Remove-Event -EventIdentifier $eventItem.EventIdentifier
  }
  if(($tick++ % 5)-eq 0) {
   $alive=Get-Process -Id $DriverId -ErrorAction SilentlyContinue
   Emit @{kind='heartbeat';driver_alive=($null -ne $alive);session=(Get-Process -Id $PID).SessionId}
   if($null -eq $alive){Emit @{kind='driver_disappeared'};break}
  }
 }
 PollHandles
 foreach($logName in @('System','Application')) {
  try {
   $events=Get-WinEvent -FilterHashtable @{LogName=$logName;StartTime=$begin.ToLocalTime();Id=@(41,42,107,1000,1001,1074,6005,6006,6008)} -ErrorAction Stop
   foreach($item in $events){Emit @{kind='os_event';log=$logName;event_id=$item.Id;record_id=$item.RecordId;provider=$item.ProviderName;created=$item.TimeCreated.ToUniversalTime().ToString('o')}}
  } catch {Emit @{kind='os_event_query';log=$logName;result=$_.Exception.Message}}
 }
 try {
  $sessionEvents=Get-WinEvent -FilterHashtable @{LogName='Microsoft-Windows-TerminalServices-LocalSessionManager/Operational';StartTime=$begin.ToLocalTime();Id=@(21,22,23,24,25)} -ErrorAction Stop
  foreach($item in $sessionEvents){Emit @{kind='session_event';event_id=$item.Id;record_id=$item.RecordId;created=$item.TimeCreated.ToUniversalTime().ToString('o')}}
 } catch {Emit @{kind='session_event_query';result=$_.Exception.Message}}
 Emit @{kind='monitor_finish';stop_requested=(Test-Path -LiteralPath $StopPath)}
} finally {foreach($id in $sources){Unregister-Event -SourceIdentifier $id -ErrorAction SilentlyContinue};$stream.Dispose()}
