//! Running independent games at once.
//!
//! The engine exists to be simulated against: benchmarks, soaks, tournaments,
//! and self-play all play a batch of games that share nothing but the
//! ruleset, which is immutable. A batch is therefore embarrassingly parallel,
//! and running it on one core left most of the machine idle.
//!
//! Every game is still seeded and deterministic, and results are returned in
//! job order, so a batch produces exactly what it produced serially — only
//! sooner.

use std::any::Any;
use std::cell::Cell;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;

/// Stack a worker gets. Turn resolution nests deeply enough that a whole game
/// does not fit in a thread's stock 2 MiB, which is a quarter of the 8 MiB the
/// main thread hands the serial path: a batch that completed serially aborted
/// with a stack overflow as soon as it was spread across workers, and the
/// larger the armies grew the sooner it happened.
const WORKER_STACK: usize = 32 * 1024 * 1024;

type Task = Box<dyn FnOnce() + Send + 'static>;

enum WorkerMessage {
    Run(Task),
    Shutdown,
}

enum BatchMessage<T> {
    Value(usize, T),
    Panicked(Box<dyn Any + Send + 'static>),
    WorkerDone,
}

thread_local! {
    /// Nested fork-join is both expensive and capable of deadlocking a bounded
    /// pool when every worker waits for work queued behind itself. A job that
    /// reaches another parallel frontier therefore evaluates that inner
    /// frontier serially.
    static IN_WORK_POOL: Cell<bool> = const { Cell::new(false) };
}

/// Persistent workers for the fine-grained, read-only phases of one game.
///
/// Unlike [`map`], which deliberately creates isolated workers for one batch
/// of complete games, a `WorkPool` lives across thousands of AI decisions.
/// Each call hands out indices dynamically and returns values in index order,
/// so worker completion order can never change an action tie-break or replay.
/// Jobs own their inputs (`'static`) because a persistent worker cannot safely
/// borrow the caller's stack; game evaluators pass an `Arc` snapshot or an
/// owned search branch instead.
pub struct WorkPool {
    sender: mpsc::Sender<WorkerMessage>,
    workers: Vec<JoinHandle<()>>,
}

impl WorkPool {
    /// Start a reusable pool. Zero is treated as one so a CLI-provided worker
    /// count always has a deterministic serial fallback.
    pub fn new(threads: usize) -> WorkPool {
        let threads = threads.max(1);
        let (sender, receiver) = mpsc::channel::<WorkerMessage>();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(threads);
        for index in 0..threads {
            let receiver = Arc::clone(&receiver);
            let worker = std::thread::Builder::new()
                .name(format!("civvis-work-{index}"))
                .stack_size(WORKER_STACK)
                .spawn(move || {
                    IN_WORK_POOL.with(|inside| inside.set(true));
                    loop {
                        // `Receiver` is not Sync. Hold the lock only long
                        // enough to claim one task; executing outside it lets
                        // all workers drain a queued batch concurrently.
                        let message = receiver
                            .lock()
                            .expect("a work-pool receiver was poisoned")
                            .recv();
                        match message {
                            Ok(WorkerMessage::Run(task)) => task(),
                            Ok(WorkerMessage::Shutdown) | Err(_) => break,
                        }
                    }
                    IN_WORK_POOL.with(|inside| inside.set(false));
                })
                .expect("the operating system refused a persistent worker thread");
            workers.push(worker);
        }
        WorkPool { sender, workers }
    }

    /// Number of reusable workers owned by this pool.
    pub fn threads(&self) -> usize {
        self.workers.len()
    }

    /// Evaluate `count` owned jobs and return their results in index order.
    ///
    /// At most one batch task per worker is queued. Those tasks claim indices
    /// from a shared counter, retaining load balance without one allocation and
    /// one channel message per candidate. A panic cancels unclaimed work,
    /// drains every active worker, and resumes on the caller; the pool itself
    /// remains usable.
    pub fn map<T, F>(&self, count: usize, job: F) -> Vec<T>
    where
        T: Send + 'static,
        F: Fn(usize) -> T + Send + Sync + 'static,
    {
        if count == 0 {
            return Vec::new();
        }
        let nested = IN_WORK_POOL.with(Cell::get);
        let active = self.workers.len().min(count);
        if active == 1 || nested {
            return (0..count).map(job).collect();
        }

        let job = Arc::new(job);
        let next = Arc::new(AtomicUsize::new(0));
        let cancelled = Arc::new(AtomicBool::new(false));
        let (result_tx, result_rx) = mpsc::channel::<BatchMessage<T>>();

        for _ in 0..active {
            let job = Arc::clone(&job);
            let next = Arc::clone(&next);
            let cancelled = Arc::clone(&cancelled);
            let result_tx = result_tx.clone();
            self.sender
                .send(WorkerMessage::Run(Box::new(move || {
                    while !cancelled.load(Ordering::Acquire) {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index >= count {
                            break;
                        }
                        match catch_unwind(AssertUnwindSafe(|| job(index))) {
                            Ok(value) => {
                                if result_tx.send(BatchMessage::Value(index, value)).is_err() {
                                    break;
                                }
                            }
                            Err(payload) => {
                                cancelled.store(true, Ordering::Release);
                                let _ = result_tx.send(BatchMessage::Panicked(payload));
                                break;
                            }
                        }
                    }
                    let _ = result_tx.send(BatchMessage::WorkerDone);
                })))
                .expect("the persistent work pool stopped unexpectedly");
        }
        drop(result_tx);

        let mut values: Vec<Option<T>> = (0..count).map(|_| None).collect();
        let mut finished = 0;
        let mut first_panic = None;
        while finished < active {
            match result_rx
                .recv()
                .expect("a persistent worker stopped without reporting completion")
            {
                BatchMessage::Value(index, value) => values[index] = Some(value),
                BatchMessage::Panicked(payload) => {
                    if first_panic.is_none() {
                        first_panic = Some(payload);
                    }
                }
                BatchMessage::WorkerDone => finished += 1,
            }
        }
        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
        values
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                value.unwrap_or_else(|| panic!("work-pool job {index} produced no result"))
            })
            .collect()
    }
}

impl Drop for WorkPool {
    fn drop(&mut self) {
        for _ in &self.workers {
            let _ = self.sender.send(WorkerMessage::Shutdown);
        }
        let current = std::thread::current().id();
        for worker in self.workers.drain(..) {
            // An `Arc<WorkPool>` may exceptionally lose its last owner inside
            // one of its jobs. Dropping that thread's own handle detaches it;
            // the queued shutdown still lets the worker exit after this task.
            if worker.thread().id() != current {
                let _ = worker.join();
            }
        }
    }
}

/// Spawn one worker with a stack a whole game fits in.
fn spawn_worker<'scope, 'env, F>(scope: &'scope std::thread::Scope<'scope, 'env>, worker: F)
where
    F: FnOnce() + Send + 'scope,
{
    std::thread::Builder::new()
        .stack_size(WORKER_STACK)
        .spawn_scoped(scope, worker)
        .expect("the operating system refused a worker thread");
}

/// How many jobs to run at once by default: one per core.
pub fn default_jobs() -> usize {
    std::thread::available_parallelism()
        .map(|cores| cores.get())
        .unwrap_or(1)
}

/// Run `count` independent jobs across `jobs` threads, returning their
/// results in index order.
///
/// Jobs are handed out one at a time rather than in equal blocks, because
/// games of the same length are not games of the same cost — one that ends in
/// an early conquest is far cheaper than one that runs to the turn limit.
pub fn map<T, F>(count: usize, jobs: usize, job: F) -> Vec<T>
where
    T: Send,
    F: Fn(usize) -> T + Sync,
{
    let threads = jobs.clamp(1, count.max(1));
    if threads == 1 {
        return (0..count).map(job).collect();
    }
    let next = AtomicUsize::new(0);
    let slots: Vec<Mutex<Option<T>>> = (0..count).map(|_| Mutex::new(None)).collect();
    let slots = &slots;
    let job = &job;
    let next = &next;
    std::thread::scope(|scope| {
        for _ in 0..threads {
            spawn_worker(scope, move || loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= count {
                    break;
                }
                let value = job(index);
                *slots[index].lock().expect("a job panicked mid-write") = Some(value);
            });
        }
    });
    slots
        .iter()
        .enumerate()
        .map(|(index, slot)| {
            slot.lock()
                .expect("a job panicked mid-write")
                .take()
                .unwrap_or_else(|| panic!("job {index} produced no result"))
        })
        .collect()
}

/// Like [`map`], but reports each result as soon as every job before it has
/// finished, so a long batch still prints in order as it goes.
pub fn map_reporting<T, F, R>(count: usize, jobs: usize, job: F, mut report: R) -> Vec<T>
where
    T: Send + Clone,
    F: Fn(usize) -> T + Sync,
    R: FnMut(usize, &T) + Send,
{
    let threads = jobs.clamp(1, count.max(1));
    if threads == 1 {
        return (0..count)
            .map(|index| {
                let value = job(index);
                report(index, &value);
                value
            })
            .collect();
    }
    let next = AtomicUsize::new(0);
    let done: Vec<Mutex<Option<T>>> = (0..count).map(|_| Mutex::new(None)).collect();
    let reported = Mutex::new(0usize);
    let report = Mutex::new(&mut report);
    let (done, reported, report, next, job) = (&done, &reported, &report, &next, &job);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            spawn_worker(scope, move || loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= count {
                    break;
                }
                *done[index].lock().expect("a job panicked mid-write") = Some(job(index));
                // Flush whatever prefix is now complete. Holding the counter
                // while reporting keeps the order, and only one thread can be
                // inside this at a time.
                let mut cursor = reported.lock().expect("a job panicked mid-report");
                while *cursor < count {
                    let ready = done[*cursor].lock().expect("a job panicked mid-write");
                    let Some(value) = ready.as_ref() else { break };
                    report.lock().expect("a job panicked mid-report")(*cursor, value);
                    *cursor += 1;
                }
            });
        }
    });
    done.iter()
        .map(|slot| {
            slot.lock()
                .expect("a job panicked mid-write")
                .take()
                .expect("every job produced a result")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::WorkPool;
    use std::collections::HashSet;
    use std::panic::AssertUnwindSafe;
    use std::sync::{Arc, Barrier};

    #[test]
    fn results_come_back_in_job_order() {
        let squares = super::map(64, 8, |index| index * index);
        assert_eq!(squares, (0..64).map(|i| i * i).collect::<Vec<_>>());
    }

    #[test]
    fn one_thread_is_the_serial_path() {
        assert_eq!(super::map(5, 1, |index| index + 1), [1, 2, 3, 4, 5]);
    }

    #[test]
    fn reports_in_order_however_jobs_finish() {
        let seen = std::sync::Mutex::new(Vec::new());
        let values = super::map_reporting(
            32,
            8,
            |index| {
                // Reverse the natural finishing order as far as the scheduler
                // allows, so in-order reporting is actually doing work.
                std::thread::sleep(std::time::Duration::from_micros((32 - index as u64) * 50));
                index
            },
            |index, value| seen.lock().unwrap().push((index, *value)),
        );
        assert_eq!(values, (0..32).collect::<Vec<_>>());
        assert_eq!(
            seen.into_inner().unwrap(),
            (0..32).map(|i| (i, i)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn persistent_pool_orders_results_and_reuses_workers() {
        let pool = WorkPool::new(4);
        let run = |pool: &WorkPool| {
            let barrier = Arc::new(Barrier::new(4));
            pool.map(4, move |index| {
                barrier.wait();
                (index, std::thread::current().id())
            })
        };
        let first = run(&pool);
        let second = run(&pool);
        assert_eq!(
            first.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        let first_workers = first
            .into_iter()
            .map(|(_, worker)| worker)
            .collect::<HashSet<_>>();
        let second_workers = second
            .into_iter()
            .map(|(_, worker)| worker)
            .collect::<HashSet<_>>();
        assert_eq!(first_workers.len(), 4);
        assert_eq!(first_workers, second_workers);
    }

    #[test]
    fn nested_pool_work_uses_its_current_worker_serially() {
        let pool = Arc::new(WorkPool::new(2));
        let nested_pool = Arc::clone(&pool);
        let results = pool.map(2, move |_| {
            let current = std::thread::current().id();
            let inner = Arc::clone(&nested_pool);
            inner.map(8, move |_| std::thread::current().id() == current)
        });
        assert!(results.into_iter().flatten().all(|same_worker| same_worker));
    }

    #[test]
    fn a_panicking_job_does_not_destroy_the_pool() {
        let pool = WorkPool::new(4);
        let failed = std::panic::catch_unwind(AssertUnwindSafe(|| {
            pool.map(32, |index| {
                assert_ne!(index, 7, "intentional work-pool test panic");
                index
            })
        }));
        assert!(failed.is_err());
        assert_eq!(pool.map(5, |index| index + 10), [10, 11, 12, 13, 14]);
    }

    #[test]
    fn persistent_pool_has_serial_and_empty_paths() {
        let pool = WorkPool::new(0);
        assert_eq!(pool.threads(), 1);
        assert_eq!(pool.map(3, |index| index * 2), [0, 2, 4]);
        assert!(pool.map::<usize, _>(0, |_| unreachable!()).is_empty());
    }
}
