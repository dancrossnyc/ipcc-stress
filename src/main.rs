use nix::sys::signal::SIGHUP;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

static NUMSIG: AtomicUsize = AtomicUsize::new(0);

fn initsighup() {
    extern "C" fn onhup(_: std::ffi::c_int) {
        NUMSIG.fetch_add(1, Ordering::Relaxed);
    }

    use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, sigaction};

    let sighup = SigHandler::Handler(onhup);
    let sa = SigAction::new(sighup, SaFlags::empty(), SigSet::empty());
    unsafe {
        sigaction(SIGHUP, &sa).expect("installed SIGHUP handler");
    }
}

fn threadrot() {
    use attest_data::messages::{self, HostToRotCommand};

    let handle = libipcc::IpccHandle::new().expect("Got IPCC handle");
    let mut request = [0; attest_data::messages::MAX_REQUEST_SIZE];
    let len = messages::serialize(&mut request, &HostToRotCommand::GetTqCertificates, |_| 0)
        .expect("Serialized GetTqCertificates command");
    loop {
        let mut response = [0; libipcc::ffi::IPCC_MAX_DATA_SIZE];
        let _ = handle.rot_request(&request[..len], &mut response[..]);
    }
}

fn killthr<T>(id: &JoinHandle<T>) {
    use std::os::unix::thread::JoinHandleExt;
    let ptid = id.as_pthread_t();
    let _r = nix::sys::pthread::pthread_kill(ptid, SIGHUP);
}

fn main() {
    initsighup();

    let ct = thread::spawn(threadrot);
    let mut k = 0_usize;
    let seq = [1, 2, 3, 40, 3, 2, 10, 20];
    for &delay in seq.iter().cycle() {
        for _ in 0..delay {
            thread::sleep(Duration::from_millis(1));
            k = k.wrapping_add(1);
            if k.is_multiple_of(5000) {
                let num = NUMSIG.load(Ordering::Relaxed);
                println!("iteration {k} signals received: {num}");
            }
        }
        killthr(&ct);
    }
}
