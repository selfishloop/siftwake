# OS lifetime high-water marks include short peaks between phase samples.
function Initialize-FdbProcessMemoryCounters {
    if($env:OS -ne 'Windows_NT' -or ('FdbProcessMemoryCounters' -as [type])){return}
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class FdbProcessMemoryCounters {
    [StructLayout(LayoutKind.Sequential)]
    public struct Counters {
        public uint cb, PageFaultCount;
        public UIntPtr PeakWorkingSetSize, WorkingSetSize;
        public UIntPtr QuotaPeakPagedPoolUsage, QuotaPagedPoolUsage;
        public UIntPtr QuotaPeakNonPagedPoolUsage, QuotaNonPagedPoolUsage;
        public UIntPtr PagefileUsage, PeakPagefileUsage, PrivateUsage;
    }
    [DllImport("psapi.dll", SetLastError=true)]
    public static extern bool GetProcessMemoryInfo(IntPtr process, ref Counters counters, uint size);
}
'@
}
function Get-FdbProcessMemoryPeaks([IntPtr]$ProcessHandle) {
    if($env:OS -ne 'Windows_NT'){return $null}
    $counters=[FdbProcessMemoryCounters+Counters]::new()
    $counters.cb=[System.Runtime.InteropServices.Marshal]::SizeOf($counters)
    if(-not [FdbProcessMemoryCounters]::GetProcessMemoryInfo($ProcessHandle,[ref]$counters,$counters.cb)){
        throw "OS lifetime memory counters unavailable: $([System.Runtime.InteropServices.Marshal]::GetLastWin32Error())"
    }
    [PSCustomObject]@{
        peak_private_commit_bytes=$counters.PeakPagefileUsage.ToUInt64()
        peak_working_set_bytes=$counters.PeakWorkingSetSize.ToUInt64()
        scope='entire process lifetime, read after exit with retained handle; not phase-specific'
        private_counter='PeakPagefileUsage: private committed bytes, not physical RAM'
        working_set_counter='PeakWorkingSetSize: resident pages including shared image pages'
        sample_gaps=$false
    }
}
