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
    pub active: BTreeMap<Instant, Vec<ToyWake>>
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
        add_active(item.at, item.wake, &mut self.active)
    }

    pub fn fire(&mut self, key: Instant) {
        self.active.remove(&key).unwrap().iter().for_each(
            |waker| {
                waker.wake();
            }
        )
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

fn add_active (property: Instant, new_value: ToyWake, active: &mut BTreeMap<Instant, Vec<ToyWake>>) {
    if active.contains_key(&property) {
        if let Some(x) = active.get_mut(&property) {
            (*x).push(new_value);
        } 
    } else {
        active.insert(property, vec![new_value]);
    }
}
