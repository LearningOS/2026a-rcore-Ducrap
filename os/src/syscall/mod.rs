/*!
Dispatch user syscalls and count each call before executing its handler.
Assistance: Codex; reference: rCore Guide, chapter 3 exercise.
*/

/// write syscall
const SYSCALL_WRITE: usize = 64;
/// exit syscall
const SYSCALL_EXIT: usize = 93;
/// yield syscall
const SYSCALL_YIELD: usize = 124;
/// gettime syscall
const SYSCALL_GET_TIME: usize = 169;
/// trace syscall
const SYSCALL_TRACE: usize = 410;

mod fs;
mod process;

use fs::*;
use process::*;

fn syscall_slot(id: usize) -> Option<usize> {
    match id {
        SYSCALL_WRITE => Some(0),
        SYSCALL_EXIT => Some(1),
        SYSCALL_YIELD => Some(2),
        SYSCALL_GET_TIME => Some(3),
        SYSCALL_TRACE => Some(4),
        _ => None,
    }
}

/// handle syscall exception with `syscall_id` and other arguments
pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize {
    if let Some(slot) = syscall_slot(syscall_id) {
        crate::task::record_syscall(slot);
    }
    match syscall_id {
        SYSCALL_WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        SYSCALL_EXIT => sys_exit(args[0] as i32),
        SYSCALL_YIELD => sys_yield(),
        SYSCALL_GET_TIME => sys_get_time(args[0] as *mut TimeVal, args[1]),
        SYSCALL_TRACE => sys_trace(args[0], args[1], args[2]),
        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}
