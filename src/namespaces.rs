use crate::colors::*;
use crate::paths::*;
use crate::utils::parse_pid_file;
use failure::{Error, ResultExt};
use ipc_channel::ipc::{self, IpcOneShotServer, IpcReceiver, IpcSender};
use nix::errno::Errno;
use nix::sched::{setns, unshare, CloneFlags};
use nix::sys::wait::{waitpid, WaitStatus::*};
use nix::unistd::{Gid, Pid, Uid};
use nix::Error::Sys;
use serde::{Deserialize, Serialize};
use std::boxed::Box;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::io::IntoRawFd;
use std::{thread, time};
use yansi::Paint;

#[derive(Serialize, Deserialize, Debug)]
struct Ready;

// TODO(cleanup): Seems to me like from the glibc docs of clone, a
// stack for the child should only be necessary if CLONE_VM is set.
// Also, 1mb is certainly overkill.
const STACK_SIZE: usize = 1024 * 1024;

pub fn with_unshared_mount<F>(mut child_fn: F) -> Result<Pid, Error>
where
    F: FnMut() -> Result<(), Error>,
{
    let clone_flags = CloneFlags::CLONE_NEWNS;
    let child_stack: &mut [u8; STACK_SIZE] = &mut [0; STACK_SIZE];
    let child_pid = ::nix::sched::clone(
        Box::new(|| {
            match child_fn() {
                // Exited successfully.
                Ok(()) => 0,
                Err(err) => {
                    println!();
                    println!("{} {}", color_err(&"mzr child error:"), err);
                    1
                }
            }
        }),
        child_stack,
        clone_flags,
        None,
    ).context("Error while cloning mzr child with unshared mount namespace.")?;
    Ok(child_pid)
}

pub fn with_unshared_user_and_mount<F, G>(
    mut write_maps_fn: F,
    mut child_fn: G,
) -> Result<Pid, Error>
where
    F: FnMut(Pid) -> Result<(), Error>,
    G: FnMut() -> Result<(), Error>,
{
    // clone with unshared mount and user namespaces.
    let clone_flags = CloneFlags::CLONE_NEWNS | CloneFlags::CLONE_NEWUSER;
    let child_stack: &mut [u8; STACK_SIZE] = &mut [0; STACK_SIZE];
    let (parent_server, parent_name) = init_ipc()?;
    let child_pid = ::nix::sched::clone(
        Box::new(|| {
            // Wait for ready message that UID mapping has been setup before
            // running child_fn. Otherwise, mounting will fail. Also, if the
            // child process attempts to exec before the UID mapping has been
            // setup, then the child will lose its capabilities (see
            // "capabilities(7)" man page).
            match recv_ready(&parent_name).and(child_fn()) {
                // Exited successfully.
                Ok(()) => 0,
                Err(err) => {
                    println!();
                    println!("{} {}", color_err(&"mzr child error:"), err);
                    1
                }
            }
        }),
        child_stack,
        clone_flags,
        None,
    ).context("Error while cloning mzr child with unshared user and mount namespaces.")?;
    write_maps_fn(child_pid)?;
    send_ready(parent_server)?;
    Ok(child_pid)
}

// IPC helper functions

fn init_ipc() -> Result<(IpcOneShotServer<IpcSender<Ready>>, String), Error> {
    wrap_ipc(IpcOneShotServer::new().map_err(|x| x.into()))
}

// TODO(cleanup): Made up this idiom of using an argumentless closure to still
// use the "?" error plumbing, while having a helper that modifies the error
// contents.  Is there a cleaner way to do something like this?

fn send_ready(parent_server: IpcOneShotServer<IpcSender<Ready>>) -> Result<(), Error> {
    wrap_ipc({
        let (_, tx1): (_, IpcSender<Ready>) = parent_server.accept()?;
        tx1.send(Ready)?;
        Ok(())
    })
}

fn recv_ready(parent_name: &str) -> Result<(), Error> {
    wrap_ipc({
        // Establish a connection with the parent.
        let (tx1, rx1): (IpcSender<Ready>, IpcReceiver<Ready>) = ipc::channel()?;
        let tx0 = IpcSender::connect(parent_name.to_string())?;
        tx0.send(tx1)?;
        let Ready = rx1.recv()?;
        Ok(())
    })
}

fn wrap_ipc<T>(x: Result<T, Error>) -> Result<T, Error> {
    Ok(x.context("Error encountered in interprocess communication mechanism.")?)
}

pub fn map_user_to_root(child_process: Pid, user: Uid, group: Gid) -> Result<(), Error> {
    let root_user = Uid::from_raw(0);
    let root_group = Gid::from_raw(0);
    map_one_user_and_group(child_process, user, root_user, group, root_group)
}

pub fn map_root_to_user(child_process: Pid, user: Uid, group: Gid) -> Result<(), Error> {
    let root_user = Uid::from_raw(0);
    let root_group = Gid::from_raw(0);
    map_one_user_and_group(child_process, root_user, user, root_group, group)
}

pub fn map_one_user_and_group(
    child_process: Pid,
    source_user: Uid,
    target_user: Uid,
    source_group: Gid,
    target_group: Gid,
) -> Result<(), Error> {
    let result: Result<(), Error> = try {
        // Map current user to root within the user namespace.
        let uid_map_path = format!("/proc/{}/uid_map", child_process);
        let mut uid_map_file = OpenOptions::new().write(true).open(uid_map_path)?;
        uid_map_file.write_all(format!("{} {} 1\n", target_user, source_user).as_bytes())?;

        // Disable usage of setgroups system call, allowing gid_map to
        // be written.
        let set_groups_path = format!("/proc/{}/setgroups", child_process);
        let mut set_groups_file = OpenOptions::new().write(true).open(set_groups_path)?;
        set_groups_file.write_all(b"deny")?;

        // Map current group to root within the user namespace.
        let gid_map_path = format!("/proc/{}/gid_map", child_process);
        let mut gid_map_file = OpenOptions::new().write(true).open(gid_map_path)?;
        gid_map_file.write_all(format!("{} {} 1\n", target_group, source_group).as_bytes())?;
    };
    result.context("Error encountered while setting up child process user namespace.")?;
    Ok(())
}

/*
// TODO(cleanup)
fn wrap_user_mapping<T>(x: Result<T, Error>) -> Result<T, Error> {
    Ok(x?)
}
*/

pub fn enter_daemon_space(mzr_dir: &MzrDir) -> Result<(), Error> {
    enter_user_and_mount(parse_pid_file(DaemonPidFile::new(&DaemonDir::new(
        &mzr_dir,
    )))?)
}

pub fn unshare_mount() -> Result<(), Error> {
    unshare(CloneFlags::CLONE_NEWNS)?;
    Ok(())
}

pub fn enter_mount(pid: Pid) -> Result<(), Error> {
    let proc_dir = ProcDir::new(pid);
    enter_ns(
        &ProcNamespaceFile::new_mount(&proc_dir),
        CloneFlags::CLONE_NEWNS,
    )
}

pub fn enter_user_and_mount(pid: Pid) -> Result<(), Error> {
    let proc_dir = ProcDir::new(pid);
    enter_ns(
        &ProcNamespaceFile::new_user(&proc_dir),
        CloneFlags::CLONE_NEWUSER,
    )?;
    enter_ns(
        &ProcNamespaceFile::new_mount(&proc_dir),
        CloneFlags::CLONE_NEWNS,
    )
}

fn enter_ns(ns_path: &ProcNamespaceFile, flags: CloneFlags) -> Result<(), Error> {
    // TODO(cleanup): make daemon_cmd a constant.
    let daemon_cmd_str = String::from("mzr daemon");
    let daemon_cmd = color_cmd(&daemon_cmd_str);
    let ns_file = File::open(&ns_path).context(format_err!(
        "Is {} running? Encountered unexpected error opening {}.",
        daemon_cmd,
        &ns_path
    ))?;
    setns(ns_file.into_raw_fd(), flags)?;
    Ok(())
}
