use failure::Error;
use ipc_channel::ipc::{self, IpcOneShotServer, IpcReceiver, IpcSender};
use nix::sched::CloneFlags;
use nix::sys::wait::waitpid;
use nix::unistd;
use std::boxed::Box;
use std::fs::OpenOptions;
use std::io::Write;
use std::{thread, time};
use yansi::Paint;

#[derive(Serialize, Deserialize, Debug)]
struct Ready;

pub fn with_unshared_user_and_mount<F>(mut child_fn: F) -> Result<(), Error>
where
    F: FnMut() -> Result<(), Error>,
{
    // new unshared mount namespace and a new unshared user namespace.
    let clone_flags = CloneFlags::CLONE_NEWNS | CloneFlags::CLONE_NEWUSER;

    // TODO Seems to me like from the glibc docs of clone, a stack for the child
    // should only be necessary if CLONE_VM is set.
    const STACK_SIZE: usize = 1024 * 1024;
    let ref mut child_stack: [u8; STACK_SIZE] = [0; STACK_SIZE];

    let (parent_server, parent_name) = IpcOneShotServer::new()?;
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
                    println!("{} {}", Paint::red("Error in mizer child:"), err);
                    1
                }
            }
        }),
        child_stack,
        clone_flags,
        None,
    )?;

    // Map the current user to root within the child process.
    let uid_map_path = format!("/proc/{}/uid_map", child_pid);
    let mut uid_map_file = OpenOptions::new().write(true).open(uid_map_path)?;
    uid_map_file.write_all(format!("0 {} 1\n", unistd::Uid::current()).as_bytes())?;

    send_ready(parent_server)?;

    // FIXME: Why is this necessary??  Should do something more reliable.
    thread::sleep(time::Duration::from_millis(100));

    waitpid(child_pid, None)?;

    Ok(())
}

fn send_ready(parent_server: IpcOneShotServer<IpcSender<Ready>>) -> Result<(), Error> {
    let (_, tx1): (_, IpcSender<Ready>) = parent_server.accept()?;
    tx1.send(Ready)?;
    Ok(())
}

fn recv_ready(parent_name: &String) -> Result<(), Error> {
    // Establish a connection with the parent.
    let (tx1, rx1): (IpcSender<Ready>, IpcReceiver<Ready>) = ipc::channel()?;
    let tx0 = IpcSender::connect(parent_name.to_string())?;
    tx0.send(tx1)?;
    let Ready = rx1.recv()?;
    Ok(())
}

/*
fn ipc_error<T>(x: Result<T, bincode::Error>) -> Result<T, Error> {
    x.map_err(|e| format_err!("Error in interprocess communication: {}", e)).map(|x| ())
}
*/
