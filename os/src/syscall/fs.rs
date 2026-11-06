/*
 * Checked metadata copying and hard-link syscalls, implemented with Codex.
 * Contract: https://learningos.cn/rCore-Tutorial-Guide/chapter6/4exercise.html
 */
use crate::config::PAGE_SIZE;
use crate::fs::{link_file, open_file, unlink_file, OpenFlags, Stat};
use crate::mm::{translated_byte_buffer, translated_str, PageTable, UserBuffer, VirtAddr};
use crate::task::{current_task, current_user_token};
use alloc::string::String;
use alloc::vec::Vec;

fn valid_user_range(token: usize, start: usize, len: usize, write: bool) -> bool {
    let Some(end) = start.checked_add(len) else {
        return false;
    };
    // Only the lower canonical SV39 range contains process-owned user memory.
    if start >= 1usize << 38 || end > 1usize << 38 {
        return false;
    }
    let table = PageTable::from_token(token);
    let mut address = start;
    while address < end {
        let Some(entry) = table.translate(VirtAddr::from(address).floor()) else {
            return false;
        };
        let user_accessible = entry.bits & (1 << 4) != 0;
        if !entry.is_valid()
            || !user_accessible
            || if write {
                !entry.writable()
            } else {
                !entry.readable()
            }
        {
            return false;
        }
        address = ((address / PAGE_SIZE + 1) * PAGE_SIZE).min(end);
    }
    true
}

/// Copy initialized bytes only after validating the entire writable user range.
pub(super) fn copy_to_user(token: usize, destination: usize, bytes: &[u8]) -> bool {
    if !valid_user_range(token, destination, bytes.len(), true) {
        return false;
    }
    let table = PageTable::from_token(token);
    let mut copied = 0;
    while copied < bytes.len() {
        let address = destination + copied;
        let offset = address % PAGE_SIZE;
        let count = (PAGE_SIZE - offset).min(bytes.len() - copied);
        let page = table
            .translate(VirtAddr::from(address).floor())
            .unwrap()
            .ppn();
        page.get_bytes_array()[offset..offset + count]
            .copy_from_slice(&bytes[copied..copied + count]);
        copied += count;
    }
    true
}

pub(super) fn user_name(token: usize, name: *const u8) -> Option<String> {
    let table = PageTable::from_token(token);
    let mut bytes = Vec::new();
    for offset in 0..=27 {
        let address = (name as usize).checked_add(offset)?;
        if !valid_user_range(token, address, 1, false) {
            return None;
        }
        let page = table.translate(VirtAddr::from(address).floor())?.ppn();
        let byte = page.get_bytes_array()[address % PAGE_SIZE];
        if byte == 0 {
            return String::from_utf8(bytes).ok();
        }
        bytes.push(byte);
    }
    None
}

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_write", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_read", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    trace!("kernel:pid[{}] sys_open", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    trace!("kernel:pid[{}] sys_close", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

/// Return a snapshot of the file's inode metadata.
pub fn sys_fstat(fd: usize, st: *mut Stat) -> isize {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let Some(Some(file)) = inner.fd_table.get(fd) else {
        return -1;
    };
    let file = file.clone();
    let token = inner.memory_set.token();
    drop(inner);
    let Some(stat) = file.stat() else {
        return -1;
    };
    // Serialize fields explicitly so no uninitialized Rust padding reaches userspace.
    let mut bytes = [0u8; core::mem::size_of::<Stat>()];
    bytes[..8].copy_from_slice(&stat.dev.to_ne_bytes());
    bytes[8..16].copy_from_slice(&stat.ino.to_ne_bytes());
    bytes[16..20].copy_from_slice(&stat.mode.bits().to_ne_bytes());
    bytes[20..24].copy_from_slice(&stat.nlink.to_ne_bytes());
    if copy_to_user(token, st as usize, &bytes) {
        0
    } else {
        -1
    }
}

/// Add a second root-directory name for an existing file.
pub fn sys_linkat(old_name: *const u8, new_name: *const u8) -> isize {
    let token = current_user_token();
    let Some(old_name) = user_name(token, old_name) else {
        return -1;
    };
    let Some(new_name) = user_name(token, new_name) else {
        return -1;
    };
    if link_file(&old_name, &new_name) {
        0
    } else {
        -1
    }
}

/// Remove a name while retaining any open file handles.
pub fn sys_unlinkat(name: *const u8) -> isize {
    let token = current_user_token();
    let Some(name) = user_name(token, name) else {
        return -1;
    };
    if unlink_file(&name) {
        0
    } else {
        -1
    }
}
