//! Benchmark-only counters for the calling thread, outside the wall-time bracket.
//! Cycles are not a fixed-frequency clock; CPU time can be coarsely quantized.
#[derive(Clone, Copy)]
pub struct Observation {
    pub cycles: u64,
    pub cpu_100ns: u64,
}

pub fn read() -> Option<Observation> {
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        #[repr(C)]
        #[derive(Default)]
        struct FileTime {
            low: u32,
            high: u32,
        }
        impl FileTime {
            fn ticks(&self) -> u64 {
                (u64::from(self.high) << 32) | u64::from(self.low)
            }
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentThread() -> *mut c_void;
            fn QueryThreadCycleTime(thread: *mut c_void, cycles: *mut u64) -> i32;
            fn GetThreadTimes(
                thread: *mut c_void,
                created: *mut FileTime,
                exited: *mut FileTime,
                kernel: *mut FileTime,
                user: *mut FileTime,
            ) -> i32;
        }
        let (mut created, mut exited, mut kernel, mut user) = (
            FileTime::default(),
            FileTime::default(),
            FileTime::default(),
            FileTime::default(),
        );
        let mut cycles = 0;
        // SAFETY: all output pointers are live, correctly laid out values, and
        // the pseudo-handle names this thread for these synchronous calls.
        let ok = unsafe {
            let thread = GetCurrentThread();
            QueryThreadCycleTime(thread, &mut cycles) != 0
                && GetThreadTimes(thread, &mut created, &mut exited, &mut kernel, &mut user) != 0
        };
        ok.then_some(Observation {
            cycles,
            cpu_100ns: kernel.ticks() + user.ticks(),
        })
    }
    #[cfg(not(windows))]
    None
}
