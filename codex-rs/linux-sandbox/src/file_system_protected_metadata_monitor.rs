use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use crate::file_system_protected_metadata::ProtectedCreateTarget;
use crate::file_system_protected_metadata_cleanup::ViolationReporter;
use crate::file_system_protected_metadata_cleanup::remove_protected_create_target_best_effort;

pub(crate) struct CreateMonitor {
    stop: Arc<AtomicBool>,
    violation: Arc<AtomicBool>,
    handle: thread::JoinHandle<()>,
}

struct CreateWatcher {
    fd: libc::c_int,
    _watches: Vec<libc::c_int>,
}

impl CreateMonitor {
    pub(crate) fn start(
        targets: &[ProtectedCreateTarget],
        report_violation: ViolationReporter,
    ) -> Option<Self> {
        if targets.is_empty() {
            return None;
        }

        let targets = targets.to_vec();
        let stop = Arc::new(AtomicBool::new(false));
        let violation = Arc::new(AtomicBool::new(false));
        let monitor_stop = Arc::clone(&stop);
        let monitor_violation = Arc::clone(&violation);
        let handle = thread::spawn(move || {
            let watcher = CreateWatcher::new(&targets);
            while !monitor_stop.load(Ordering::SeqCst) {
                for target in &targets {
                    if remove_protected_create_target_best_effort(target, report_violation) {
                        monitor_violation.store(true, Ordering::SeqCst);
                    }
                }
                if let Some(watcher) = &watcher {
                    watcher.wait_for_create_event(&monitor_stop);
                } else {
                    thread::sleep(Duration::from_millis(1));
                }
            }
        });

        Some(Self {
            stop,
            violation,
            handle,
        })
    }

    pub(crate) fn stop(self) -> bool {
        self.stop.store(true, Ordering::SeqCst);
        self.handle
            .join()
            .unwrap_or_else(|_| panic!("protected create monitor thread panicked"));
        self.violation.load(Ordering::SeqCst)
    }
}

impl CreateWatcher {
    fn new(targets: &[ProtectedCreateTarget]) -> Option<Self> {
        let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if fd < 0 {
            return None;
        }

        let mut watched_parents = Vec::<PathBuf>::new();
        let mut watches = Vec::new();
        for target in targets {
            let Some(parent) = target.path().parent() else {
                continue;
            };
            if watched_parents.iter().any(|watched| watched == parent) {
                continue;
            }
            watched_parents.push(parent.to_path_buf());
            let Ok(parent_cstr) = CString::new(parent.as_os_str().as_bytes()) else {
                continue;
            };
            let mask =
                libc::IN_CREATE | libc::IN_MOVED_TO | libc::IN_DELETE_SELF | libc::IN_MOVE_SELF;
            let watch = unsafe { libc::inotify_add_watch(fd, parent_cstr.as_ptr(), mask) };
            if watch >= 0 {
                watches.push(watch);
            }
        }

        if watches.is_empty() {
            unsafe {
                libc::close(fd);
            }
            return None;
        }

        Some(Self {
            fd,
            _watches: watches,
        })
    }

    fn wait_for_create_event(&self, stop: &AtomicBool) {
        let mut poll_fd = libc::pollfd {
            fd: self.fd,
            events: libc::POLLIN,
            revents: 0,
        };
        while !stop.load(Ordering::SeqCst) {
            let res = unsafe { libc::poll(&mut poll_fd, 1, 10) };
            if res > 0 {
                self.drain_events();
                return;
            }
            if res == 0 {
                return;
            }
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return;
        }
    }

    fn drain_events(&self) {
        let mut buf = [0_u8; 4096];
        loop {
            let read = unsafe { libc::read(self.fd, buf.as_mut_ptr().cast(), buf.len()) };
            if read > 0 {
                continue;
            }
            if read == 0 {
                return;
            }
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return;
        }
    }
}

impl Drop for CreateWatcher {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}
