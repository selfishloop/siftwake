//! Optional diagnostic counter for the calling benchmark thread only.
//! This is neither elapsed time nor a whole-process CPU/RAM measurement.
pub fn read() -> Option<u64> {
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentThread() -> *mut c_void;
            fn QueryThreadCycleTime(thread: *mut c_void, cycles: *mut u64) -> i32;
        }
        let mut cycles = 0;
        // SAFETY: the pseudo-handle refers to this live thread; the writable
        // output pointer remains valid for the duration of the synchronous call.
        (unsafe { QueryThreadCycleTime(GetCurrentThread(), &mut cycles) } != 0).then_some(cycles)
    }
    #[cfg(not(windows))]
    None
}
