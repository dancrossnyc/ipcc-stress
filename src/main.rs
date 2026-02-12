use attest_data::messages::HostToRotCommand;
use attest_data::messages::RotToHost;
use libipcc::IpccError;
use slog_error_chain::InlineErrorChain;
use libipcc::IpccHandle;
use libipcc::ffi::IPCC_MAX_DATA_SIZE;
use signal_hook::consts::SIGHUP;
use signal_hook::iterator::Signals;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

static NUM_SIGHUP: AtomicUsize = AtomicUsize::new(0);

fn main() {
    let expected = getcert1().expect("got rot certs");

    let signals = Signals::new([SIGHUP]).expect("created Signals");
    thread::spawn(|| {
        block_sighup_on_current_thread();
        monitor_incoming_signals();
    });
    thread::spawn(|| {
        block_sighup_on_current_thread();
        count_incoming_signals(signals);
    });
    thread::spawn(|| {
        block_sighup_on_current_thread();
        sighup_ourself_a_bunch();
    });

    loop {
        match getcert1() {
            Ok(data) => assert_eq!(data, expected),
            Err(err) => {
                println!("IPCC request failed: {}", InlineErrorChain::new(&err));
            },
        }
    }
}

fn block_sighup_on_current_thread() {
    use nix::sys::signal::{SigSet, SigmaskHow, Signal, pthread_sigmask};

    let mut sigset = SigSet::empty();
    sigset.add(Signal::SIGHUP);
    pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&sigset), None).expect("blocked SIGHUP");
}

fn sighup_ourself_a_bunch() {
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::getpid;

    loop {
        kill(getpid(), Signal::SIGHUP).expect("failed to send SIGHUP");
        thread::sleep(Duration::from_millis(1));
    }
}

fn count_incoming_signals(mut signals: Signals) {
    loop {
        for signal in signals.pending() {
            match signal {
                SIGHUP => {
                    NUM_SIGHUP.fetch_add(1, Ordering::Relaxed);
                }
                _ => {
                    unreachable!("unexpected signal: {signal}");
                }
            }
        }
    }
}

fn monitor_incoming_signals() {
    loop {
        let num = NUM_SIGHUP.load(Ordering::Relaxed);
        println!("sighups received: {num}");
        thread::sleep(Duration::from_secs(5));
    }
}

fn getcert1() -> Result<Vec<u8>, IpccError> {
    let handle = IpccHandle::new().unwrap();

    let mut rot_message = vec![0; attest_data::messages::MAX_REQUEST_SIZE];
    let mut rot_resp = vec![0; IPCC_MAX_DATA_SIZE];
    let len = attest_data::messages::serialize(
        &mut rot_message,
        &HostToRotCommand::GetTqCertificates,
        |_| 0,
    )
    .unwrap();
    let len = handle.rot_request(&rot_message[..len], &mut rot_resp)?;
    let data =
        attest_data::messages::parse_response(&rot_resp[..len], RotToHost::RotTqCertificates)
            .unwrap();
    Ok(data.to_vec())
}
