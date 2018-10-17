extern crate async_rust_playground as arp;

use self::arp::*;
use std::time::{Duration, Instant};

fn main() {
    let timer = ToyTimer::new();
    let exec = ToyExec::new();
    let period = Duration::from_millis(1500);
    let next = Instant::now() + period;
    for i in 1..10 {
        exec.spawn(Periodic::new(i, period, next, timer.clone()));
    }

    exec.run()
}

struct Periodic {
    // a name for this task
    id: u64,

    // how often to "ding"
    period: Duration,

    // when the next "ding" is scheduled
    next: Instant,

    // a handle back to the timer event loop
    timer: ToyTimer,
}

impl Periodic {
    fn new(id: u64, period: Duration, next: Instant, timer: ToyTimer) -> Periodic {
        Periodic {
            id, period, timer, next
        }
    }
}

impl ToyTask for Periodic {
    fn poll(&mut self, wake: ToyWake) -> Async<()> {
        // are we ready to ding yet?
        let now = Instant::now();
        if now >= self.next {
            self.next = now + self.period;
            println!("Task {} - ding", self.id);
            return Async::Ready(())
        }

        // make sure we're registered to wake up at the next expected `ding`
        self.timer.register(self.next, wake);
        Async::Pending
    }
}