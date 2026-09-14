use super::map_reporting_with_cancellation;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Barrier;
use std::time::{Duration, Instant};

fn failure_cancels_unclaimed_work(fail_in_report: bool) {
    let cancelled = AtomicBool::new(false);
    let started = AtomicUsize::new(0);
    let first_wave = Barrier::new(2);
    let result = catch_unwind(AssertUnwindSafe(|| {
        map_reporting_with_cancellation(
            64,
            2,
            |index| {
                started.fetch_add(1, Ordering::Relaxed);
                assert!(index < 2, "unclaimed work ran after cancellation");
                first_wave.wait();
                if index == 0 {
                    assert!(fail_in_report, "job failure");
                } else {
                    // Keep the surviving worker inside its active job until
                    // cancellation is visible, then exercise its next claim.
                    let deadline = Instant::now() + Duration::from_secs(10);
                    while !cancelled.load(Ordering::Acquire) {
                        assert!(Instant::now() < deadline, "failure did not cancel work");
                        std::thread::yield_now();
                    }
                }
                index
            },
            |_, _| {
                assert!(!fail_in_report, "report failure");
            },
            &cancelled,
        )
    }));
    assert!(result.is_err(), "the caller must observe the panic");
    assert!(cancelled.load(Ordering::Acquire));
    assert_eq!(started.load(Ordering::Relaxed), 2);
}

#[test]
fn a_job_panic_cancels_unclaimed_reporting_jobs() {
    failure_cancels_unclaimed_work(false);
}

#[test]
fn a_report_panic_cancels_unclaimed_reporting_jobs() {
    failure_cancels_unclaimed_work(true);
}
