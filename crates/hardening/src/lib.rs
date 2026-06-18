/// Block core dumps and `ptrace` attachment for this process.
pub fn forbid_dumps() {
    unsafe {
        libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0);
        let no_core = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        libc::setrlimit(libc::RLIMIT_CORE, &no_core);
    }
}

/// Pin every current and future page of this process into RAM so secrets
/// cannot be paged to swap.
pub fn lock_memory() {
    unsafe {
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::getrlimit(libc::RLIMIT_MEMLOCK, &mut limit) == 0 {
            limit.rlim_cur = limit.rlim_max;
            libc::setrlimit(libc::RLIMIT_MEMLOCK, &limit);
        }

        if libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE) != 0 {
            let err = std::io::Error::last_os_error();
            eprintln!(
                "hush: mlockall failed ({err}); secrets may reach swap. \
                 Raise RLIMIT_MEMLOCK (e.g. LimitMEMLOCK=infinity in the service unit)."
            );
        }
    }
}
