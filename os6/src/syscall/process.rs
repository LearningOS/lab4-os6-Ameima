//! Process management syscalls

use crate::mm::{translated_refmut, translated_ref, translated_str};
use crate::task::{
    add_task, current_task, current_user_token, exit_current_and_run_next,
    suspend_current_and_run_next, TaskStatus,
};
use crate::fs::{open_file, OpenFlags, File};
use crate::timer::get_time_us;
use alloc::sync::Arc;
use alloc::vec::Vec;
use crate::config::MAX_SYSCALL_NUM;
use crate::task::TaskControlBlock;
use alloc::string::String;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

#[derive(Clone, Copy)]
pub struct TaskInfo {
    pub status: TaskStatus,
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    pub time: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    debug!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    current_task().unwrap().pid.0 as isize
}

/// Syscall Fork which returns 0 for child process and child_pid for parent process
pub fn sys_fork() -> isize {
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

/// Syscall Exec which accepts the elf path
pub fn sys_exec(path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}


/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let task = current_task().unwrap();
    // find a child process

    // ---- access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB lock exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after removing from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child TCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB lock automatically
}

// YOUR JOB: ???????????????????????? sys_get_time
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    let us = get_time_us();
    let ts_va = translated_refmut(current_user_token(), ts);
    *ts_va = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    0
}

// YOUR JOB: ???????????????????????? sys_task_info
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    let ti_va = translated_refmut(current_user_token(), ti);
    let current_task = current_task().unwrap();
    let ctcb = current_task.inner_exclusive_access();
    *ti_va = TaskInfo {
        status: ctcb.task_status,
        syscall_times: ctcb.task_syscall_times,
        time: get_time_us() / 1000 - ctcb.task_first_running_time.unwrap(),
    };
    0
}

// YOUR JOB: ??????sys_set_priority???????????????????????????
pub fn sys_set_priority(prio: isize) -> isize {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .set_task_priority(prio)
}

// YOUR JOB: ????????????????????? sys_mmap ??? sys_munmap
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .memory_set
        .mmap(start, len, port)
}

pub fn sys_munmap(start: usize, len: usize) -> isize {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .memory_set
        .munmap(start, len)
}


// // YOUR JOB: ?????? sys_spawn ????????????
// // ???????????????????????????
// pub fn sys_spawn(path: *const u8) -> isize {
//     let token = current_user_token();
//     let path = translated_str(token, path);
//     if let Some(data) = open_file(path.as_str(), OpenFlags::RDONLY) {
//         let new_task = Arc::new(TaskControlBlock::new(data.read_all().as_slice()));
//         let mut new_inner = new_task.inner_exclusive_access();
//         let parent = current_task().unwrap();
//         let mut parent_inner = parent.inner_exclusive_access();
//         new_inner.parent = Some(Arc::downgrade(&parent));
//         parent_inner.children.push(new_task.clone());
//         let pid = new_task.pid.0 as isize;
//         let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
//         // clone all fds from parent to child
//         for fd in parent_inner.fd_table.iter() {
//             if let Some(file) = fd {
//                 new_fd_table.push(Some(file.clone()));
//             } else {
//                 new_fd_table.push(None);
//             }
//         }
//         new_inner.fd_table = new_fd_table;
//         drop(new_inner);
//         drop(parent_inner);
//         add_task(new_task);
//         pid
//     } else {
//         -1
//     }
// }

// YOUR JOB: ?????? sys_spawn ????????????
// ALERT: ??????????????? SPAWN ??????????????????????????????????????????SPAWN != FORK + EXEC 
pub fn sys_spawn(_path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, _path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let curr_task = current_task().unwrap();
        let new_task = curr_task.spawn(all_data.as_slice());
        let new_pid = new_task.pid.0;
        add_task(new_task);
        new_pid as isize
    } else {
        -1
    }
}
