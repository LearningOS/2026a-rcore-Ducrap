/*
Implement process syscalls through checked user-address translation.
Reference: rCore Guide chapter4/7exercise.html.
*/
use crate::mm::{copy_to_user, translated_user_byte};
use crate::task::{
    change_program_brk, current_user_token, exit_current_and_run_next,
    suspend_current_and_run_next, with_current_task,
};
use crate::timer::get_time_us;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// Copy a timestamp without assuming that its fields share a physical page.
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    let now = get_time_us();
    let mut bytes = [0u8; core::mem::size_of::<TimeVal>()];
    let word = core::mem::size_of::<usize>();
    bytes[..word].copy_from_slice(&(now / 1_000_000).to_ne_bytes());
    bytes[word..].copy_from_slice(&(now % 1_000_000).to_ne_bytes());
    if copy_to_user(current_user_token(), ts as usize, &bytes).is_some() {
        0
    } else {
        -1
    }
}

/// Inspect one user byte or this task's syscall history.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    match trace_request {
        0 => translated_user_byte(current_user_token(), id, false)
            .map(|pa| *pa.get_mut::<u8>() as isize)
            .unwrap_or(-1),
        1 => match translated_user_byte(current_user_token(), id, true) {
            Some(pa) => {
                *pa.get_mut::<u8>() = data as u8;
                0
            }
            None => -1,
        },
        2 => with_current_task(|task| {
            task.syscall_counts
                .get(id)
                .map(|count| *count as isize)
                .unwrap_or(-1)
        }),
        _ => -1,
    }
}

pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    with_current_task(|task| task.memory_set.mmap(start, len, prot))
}

pub fn sys_munmap(start: usize, len: usize) -> isize {
    with_current_task(|task| task.memory_set.munmap(start, len))
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
