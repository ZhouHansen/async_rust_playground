use std::collections::BTreeMap;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use super::task_exc::*;

/// A handle to a timer, used for registering wakeups
#[derive(Clone)]
pub struct ToyTimer {
    pub tx: mpsc::Sender<Registration>,
}

/// A wakeup request
pub struct Registration {
    pub at: Instant,
    pub wake: ToyWake,
}

/// State for the worker thread that processes timer events
pub struct Worker {
    pub rx: mpsc::Receiver<Registration>,
    pub active: BTreeMap<Instant, ToyWake>
}

impl ToyTimer {
    pub fn new() -> ToyTimer {
        let (tx, rx) = mpsc::channel();
        let worker = Worker { rx, active: BTreeMap::new() };
        thread::spawn(|| worker.work());
        ToyTimer { tx }
    }

    // Register a new wakeup with this timer
    pub fn register(&self, at: Instant, wake: ToyWake) {
        self.tx.send(Registration { at, wake: wake }).unwrap();
    }
}

impl Worker {
    pub fn enroll(&mut self, item: Registration) {
        if self.active.insert(item.at, item.wake).is_some() {
            // this simple setup doesn't support multiple registrations for
            // the same instant; we'll revisit that in the next section.
            panic!("Attempted to add to registrations for the same instant")
        }
    }

    pub fn fire(&mut self, key: Instant) {
        self.active.remove(&key).unwrap().wake();
    }

    pub fn work(mut self) {
        loop {
            if let Some(first) = self.active.keys().next().cloned() {
                let now = Instant::now();
                if first <= now {
                    self.fire(first);
                } else {
                    // we're not ready to fire off `first` yet, so wait until we are
                    // (or until we get a new registration, which might be for an
                    // earlier time).
                    if let Ok(new_registration) = self.rx.recv_timeout(first - now) {
                        self.enroll(new_registration);
                    }
                }
            } else {
                // no existing registrations, so unconditionally block until
                // we receive one.
                let new_registration = self.rx.recv().unwrap();
                self.enroll(new_registration)
            }
        }
    }
}
