use std::mem;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, Thread};
use std::process;

pub enum Async<T> {
    /// Work completed with a result of type `T`.
    Ready(T),

    /// Work was blocked, and the task is set to be woken when ready
    /// to continue.
    Pending,
}

/// An independent, non-blocking computation
pub trait ToyTask {
    /// Attempt to finish executing the task, returning `Async::Pending`
    /// if the task needs to wait for an event before it can complete.
    fn poll(&mut self, waker: ToyWake) -> Async<()>;
}

pub struct TaskEntry {
    pub task: Box<ToyTask + Send>,
    pub wake: ToyWake,
}

#[derive(Clone)]
pub struct ToyWake {
    // A link back to the executor that owns the task we want to wake up.
    pub exec: ToyExec,

    // The ID for the task we want to wake up.
    pub id: usize,
}

pub trait Wake: Send + Sync {
    /// Signals that the associated task is ready to be `poll`ed again.;
    fn wake(&self);
}

impl Wake for ToyWake {
    fn wake(&self) {
        self.exec.state_mut().wake_task(self.id);
    }
}

impl From<Arc<ToyWake>> for ToyWake {
    fn from(wake: Arc<ToyWake>) -> ToyWake {
       ToyWake {
        exec: wake.exec.clone(),
        id: wake.id.clone(),           
       }
    }
}

#[derive(Clone)]
pub struct ToyExec {
    state: Arc<Mutex<ExecState>>,
}

// the internal executor state
pub struct ExecState {
    // The next available task ID.
    pub next_id: usize,

    // The complete list of tasks, keyed by ID.
    pub tasks: HashMap<usize, TaskEntry>,

    // The set of IDs for ready-to-run tasks.
    pub ready: HashSet<usize>,

    // The actual OS thread running the executor.
    pub thread: Thread,
}

impl ToyExec {
    pub fn new() -> Self {
        ToyExec {
            state: Arc::new(Mutex::new(ExecState {
                next_id: 0,
                tasks: HashMap::new(),
                ready: HashSet::new(),
                thread: thread::current(),
            })),
        }
    }

    // a convenience method for getting our hands on the executor state
    pub fn state_mut(&self) -> MutexGuard<ExecState> {
        self.state.lock().unwrap()
    }
}

impl ExecState {
    pub fn wake_task(&mut self, id: usize) {
        self.ready.insert(id);

        // *after* inserting in the ready set, ensure the executor OS
        // thread is woken up if it's not already running.
        self.thread.unpark();
    }
}

impl ToyExec {
    pub fn spawn<T>(&self, task: T)
        where T: ToyTask + Send + 'static
    {
        let mut state = self.state_mut();

        let id = state.next_id;
        state.next_id += 1;

        let wake = ToyWake { id, exec: self.clone() };
        let entry = TaskEntry {
            wake: ToyWake::from(Arc::new(wake)),
            task: Box::new(task)
        };
        state.tasks.insert(id, entry);

        // A newly-added task is considered immediately ready to run,
        // which will cause a subsequent call to `park` to immediately
        // return.
        state.wake_task(id);
    }

    pub fn run(&self) {
        loop {
            // Each time around, we grab the *entire* set of ready-to-run task IDs:
            let mut ready = mem::replace(&mut self.state_mut().ready, HashSet::new());

            // Now try to `complete` each initially-ready task:
            for id in ready.drain() {
                // We take *full ownership* of the task; if it completes, it will
                // be dropped.
                let entry = self.state_mut().tasks.remove(&id);
                if let Some(mut entry) = entry {
                    if let Async::Pending = entry.task.poll(entry.wake.clone()) {
                        // The task hasn't completed, so put it back in the table.
                        self.state_mut().tasks.insert(id, entry);
                    }

                    if self.state_mut().tasks.len() == 0 {
                        println!("No more task, we are free!");
                        process::exit(1);
                    }
                }
            }

            // We've processed all work we acquired on entry; block until more work
            // is available. If new work became available after our `ready` snapshot,
            // this will be a no-op.
            thread::park();
        }
    }    
}

