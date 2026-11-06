/*!
Process syscalls and byte-level tracing for the flat-address-space kernel.
Assistance: Codex; reference: rCore Guide, chapter 3 exercise.
*/
use crate::{
    task::{exit_current_and_run_next, suspend_current_and_run_next},
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    match trace_request {
        // Chapter 3 deliberately has no user-memory isolation yet.
        0 => unsafe { core::ptr::read_volatile(id as *const u8) as isize },
        1 => {
            unsafe { core::ptr::write_volatile(id as *mut u8, data as u8) };
            0
        }
        2 => super::syscall_slot(id)
            .map(|slot| crate::task::current_syscall_count(slot) as isize)
            .unwrap_or(0),
        _ => -1,
    }
}
