use super::*;
use pretty_assertions::assert_eq;

#[test]
fn bwrap_signal_forwarder_terminates_child_and_keeps_parent_alive() {
    let supervisor_pid = unsafe { libc::fork() };
    assert!(supervisor_pid >= 0, "failed to fork supervisor");

    if supervisor_pid == 0 {
        run_bwrap_signal_forwarder_test_supervisor();
    }

    let status = wait_for_bwrap_child(supervisor_pid);
    assert!(libc::WIFEXITED(status), "supervisor status: {status}");
    assert_eq!(libc::WEXITSTATUS(status), 0);
}

#[cfg(test)]
fn run_bwrap_signal_forwarder_test_supervisor() -> ! {
    let child_pid = unsafe { libc::fork() };
    if child_pid < 0 {
        unsafe {
            libc::_exit(2);
        }
    }

    if child_pid == 0 {
        loop {
            unsafe {
                libc::pause();
            }
        }
    }

    install_bwrap_signal_forwarders(child_pid);
    unsafe {
        libc::raise(libc::SIGTERM);
    }

    let status = wait_for_bwrap_child(child_pid);
    let child_terminated_by_sigterm =
        libc::WIFSIGNALED(status) && libc::WTERMSIG(status) == libc::SIGTERM;
    unsafe {
        libc::_exit(if child_terminated_by_sigterm { 0 } else { 1 });
    }
}
