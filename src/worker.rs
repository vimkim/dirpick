//! One replaceable pending job per worker; typing never creates more threads.
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

struct Shared<T> {
    pending: Mutex<Option<(u64, T)>>,
    wake: Condvar,
    current: AtomicU64,
    stopped: AtomicBool,
}

pub(crate) struct Worker<T> {
    shared: Arc<Shared<T>>,
}

#[derive(Clone)]
pub(crate) struct Cancellation {
    current: Arc<dyn Fn() -> bool + Send + Sync>,
}
impl Cancellation {
    pub fn cancelled(&self) -> bool {
        (self.current)()
    }
}

impl<T: Send + 'static> Worker<T> {
    pub fn new(name: &str, work: impl Fn(u64, T, Cancellation) + Send + 'static) -> Self {
        let shared = Arc::new(Shared {
            pending: Mutex::new(None),
            wake: Condvar::new(),
            current: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
        });
        let thread_shared = shared.clone();
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                loop {
                    let mut pending = thread_shared.pending.lock().unwrap();
                    while pending.is_none() && !thread_shared.stopped.load(Ordering::Acquire) {
                        pending = thread_shared.wake.wait(pending).unwrap();
                    }
                    if thread_shared.stopped.load(Ordering::Acquire) {
                        break;
                    }
                    let (id, request) = pending.take().unwrap();
                    drop(pending);
                    let check = thread_shared.clone();
                    let cancel = Cancellation {
                        current: Arc::new(move || {
                            check.stopped.load(Ordering::Acquire)
                                || check.current.load(Ordering::Acquire) != id
                        }),
                    };
                    work(id, request, cancel);
                }
            })
            .expect("cannot start filesystem worker");
        Self { shared }
    }
    pub fn submit(&self, id: u64, request: T) {
        self.shared.current.store(id, Ordering::Release);
        *self.shared.pending.lock().unwrap() = Some((id, request));
        self.shared.wake.notify_one();
    }
    pub fn cancel(&self, id: u64) {
        self.shared.current.store(id, Ordering::Release);
        self.shared.pending.lock().unwrap().take();
    }
}
impl<T> Drop for Worker<T> {
    fn drop(&mut self) {
        // Pair the predicate and notification with the waiter's mutex: shutdown
        // cannot be lost between checking `stopped` and entering Condvar::wait.
        let mut pending = self.shared.pending.lock().unwrap();
        self.shared.stopped.store(true, Ordering::Release);
        pending.take();
        self.shared.wake.notify_one();
    }
}
