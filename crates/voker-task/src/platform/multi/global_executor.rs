//! This module provides the implementation of `GlobalExecutor`,
//! which is used exclusively in multi-threaded mode.
#![expect(unsafe_code, reason = "original implementation")]

use core::cell::{Cell, UnsafeCell};
use core::marker::PhantomData;
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::ptr;
use core::fmt;
use core::task::{Poll, Waker};
use alloc::boxed::Box;

use std::thread_local;

use async_task::{Runnable, Task};
use futures_lite::FutureExt;
use futures_lite::future::poll_fn;

use voker_os::sync::{Mutex, PoisonError};
use voker_os::utils::{CachePadded, ListQueue};
use voker_os::utils::ArrayQueue;
use voker_os::sync::atomic::{AtomicBool, Ordering};
use voker_utils::extra::ArrayDeque;

use super::XorShift64Star;

// -----------------------------------------------------------------------------
// Config

/// Capacity of each worker's local task queue.
/// 
/// Using 63 ensures `crossbeam::ArrayQueue` allocates exactly 64 slots( (x+1).next_power_of_two ).
/// This balance provides good throughput while keeping cache footprint reasonable.
const WORKER_QUEUE_SIZE: usize = 63;

// -----------------------------------------------------------------------------
// GlobalExecutor

/// A global executor with work-stealing capabilities for a task pool.
/// 
/// Each task pool will have its own dedicated `GlobalExecutor`,
/// rather than sharing a single global instance.
/// 
/// Every `GlobalExecutor` maintains an internal task queue for
/// distributing tasks across multiple threads.
/// 
/// Each thread will have a `Worker` instance, which is bound to
/// the current task pool's `GlobalExecutor` when the thread is
/// created by the pool. The `Worker` then cooperates with a
/// `LocalExecutor` to run the asynchronous execution loop.
/// 
/// A `Worker` is a thread-local executor with work-stealing capabilities.
/// It steals tasks from its bound `GlobalExecutor` into its local queue
/// for execution. When both the local and global queues are empty,
/// it will also attempt to steal tasks from other threads' `Worker`
/// instances to balance workloads.
/// 
/// Since we have three task pools but the main thread only has one `Worker`,
/// the main thread's `Worker` is not bound to any specific `GlobalExecutor`.
/// Consequently, the main thread `Worker` directly pulls tasks from the caller's
/// global queue or the **last** local queue in the state. It maintains a high
/// frequency of `yield` operations to avoid blocking the main thread.
pub(super) struct GlobalExecutor<'a> {
    state: State,
    _marker: PhantomData<UnsafeCell<&'a ()>>,
}

// -----------------------------------------------------------------------------
// State

/// The internal, shared state of the executor.
/// 
/// Separating this from `GlobalExecutor` avoids lifetime parameters
/// in worker thread-local storage while maintaining safety.
struct State {
    /// Shared global queue
    queue: ListQueue<Runnable>,
    /// “Seats” for worker threads; length equals
    /// 1 (main-thread) + the number of workers
    /// 
    /// worker_num: the number of worker threads
    /// seats[..worker_num] -> Workers' local queue
    /// seats[worker_num] -> Main Executor's local queue
    seats: CachePadded<Box<[Seat]>>,
    /// Manages sleeping workers and stores their wakers;
    /// length equals the number of workers(without main thread).
    lounge: Mutex<Lounge>,
    /// Indicates whether a worker is currently being woken up.
    /// This flag ensures workers are woken one by one, preventing thundering herd.
    /// 
    /// Note: it is also `true` when all workers are already active.
    is_waking: AtomicBool,
}

// -----------------------------------------------------------------------------
// Seat

/// A "seat" representing a worker thread's position in the executor.
/// 
/// Each seat contains:
/// - A local task queue for cache-efficient task processing
/// - An occupancy flag for thread binding during initialization
/// 
/// The seat metaphor helps visualize the fixed number of workers
/// that can participate in a task pool.
/// 
/// seats[..worker_num] -> Workers' local queue
/// seats[worker_num] -> Main Executor's local queue
struct Seat {
    /// Local, bounded task queue for this worker
    /// Uses `ArrayQueue` for lock-free push/pop operations
    queue: ArrayQueue<Runnable>,
    /// Indicates whether this seat is occupied by a bound worker
    /// Set during worker initialization via atomic compare-and-swap
    occupied: AtomicBool,
}

// -----------------------------------------------------------------------------
// Runner

/// Async task executor residing in a worker thread,
/// responsible for executing tasks and work‑stealing.
/// 
/// Stored in thread‑local storage; each thread has one
/// instance.
/// 
/// Its fields are initialized when the `TaskPool` creates
/// a thread by calling `bind_local_worker`.
/// 
/// It holds a pointer to the `GlobalExecutor`.
struct Worker {
    /// Fast random number generator for random work‑stealing.
    xor_shift: XorShift64Star,
    /// Pointer to the global executor state
    state: Cell<*const State>,
    /// Pointer to the thread’s local task queue
    queue: Cell<*const ArrayQueue<Runnable>>,
    /// Index of this worker’s seat in the global executor
    seat_index: Cell<usize>,
    /// Current activity state of the worker
    /// 
    /// State transitions:
    /// - true → false: Working → Sleeping (when no tasks available)
    /// - false → true: Sleeping/Waking → Working (when task obtained)
    working: Cell<bool>,
}

thread_local! {
    // `const {}` enable a more efficient thread local implementation.
    static LOCAL_WORKER: Worker = const {
        Worker {
            xor_shift: XorShift64Star::fixed(),
            state: Cell::new(ptr::null()),
            queue: Cell::new(ptr::null()),
            seat_index: Cell::new(0),
            working: Cell::new(true),
        }
    };
}

// -----------------------------------------------------------------------------
// Sleepers

/// Manages sleeping workers and stores their wakers.
/// 
/// A worker can be in one of three states:
/// 
/// - **Working**
/// - **Waking** (transitioning from sleeping to working)
/// - **Sleeping**
/// 
/// Then a **Working** worker that fails to obtain a runnable,
/// it will transition to **Sleeping** and try obtain again.
/// If it fails again, it will return `Pending` and sleep thread.
/// 
/// When a sleeping worker is woken, it becomes **Waking**.
/// If a runnable is obtained, it becomes **Working**;
/// otherwise it returns to **Sleeping** and try obtain again,
/// If it fails again, it will return `Pending` and sleep thread.
struct Lounge {
    /// Number of workers currently sleeping (with registered wakers)
    sleeping: usize,
    /// Number of workers in waking state (transitioning from sleep)
    waking: usize,
    /// Optional wakers for each worker seat
    /// `None` indicates worker is working or waking
    /// `Some(waker)` indicates worker is sleeping
    wakers: Box<[Option<Waker>]>,
}

// -----------------------------------------------------------------------------
// Lounge Implementation

impl Lounge {
    /// Registers a waker for a transitioning worker (Working → Sleeping)
    fn insert(&mut self, id: usize, waker: &Waker) {
        debug_assert!(id < self.wakers.len());

        let old = unsafe{ self.wakers.get_unchecked_mut(id) };
        debug_assert!(old.is_none());
        *old = Some(waker.clone());

        self.sleeping += 1;
    }

    /// Updates an existing waker or registers a new one (Waking/Sleeping → Sleeping)
    /// 
    /// Returns `true` if the state changed from Waking to Sleeping,
    /// `false` if the worker was already Sleeping.
    fn update(&mut self, id: usize, waker: &Waker) -> bool {
        debug_assert!(id < self.wakers.len());

        let old = unsafe{ self.wakers.get_unchecked_mut(id) };
        match old {
            Some(w) => {
                // Sleeping → Sleeping
                w.clone_from(waker);
                false
            },
            None => {
                // Waking → Sleeping
                *old = Some(waker.clone());
                self.waking -= 1;
                self.sleeping += 1;
                true
            },
        }
    }

    /// Removes a waker (Sleeping → Working or Sleeping → Waking)
    fn remove(&mut self, id: usize) {
        debug_assert!(id < self.wakers.len());

        let old = unsafe{ self.wakers.get_unchecked_mut(id) };
        match old {
            Some(_) => {
                // Sleeping → Working
                *old = None;
                self.sleeping -= 1;
            },
            None => {
                // Waking → Working
                self.waking -= 1;
            },
        }
    }

    /// Checks if wakeup coordination is needed
    /// 
    /// Returns `true` if:
    /// - Any workers are in waking state, OR
    /// - All workers are active (sleeping == 0)
    /// 
    /// This prevents unnecessary wakeup attempts when workers
    /// are already transitioning to active state.
    #[inline(always)]
    fn is_waking(&self) -> bool {
        self.waking > 0 || self.sleeping == 0
    }

    /// Wakes a single sleeping worker if no wakeup is already in progress
    /// 
    /// This implements a "soft" wakeup strategy - only one worker
    /// is woken per available task, reducing contention.
    #[must_use]
    fn wake_one(&mut self) -> Option<Waker> {
        // Only wake a worker if no wakeup is already happening
        for item in self.wakers.iter_mut() {
            if item.is_some() {
                self.sleeping -= 1;
                self.waking += 1;
                return item.take();
            }
        }
        None
    }
}

// -----------------------------------------------------------------------------
// State Implementation

impl State {
    /// Attempts to wake a single sleeping worker if no wakeup is in progress
    /// 
    /// This method implements the thundering herd prevention:
    /// - Atomically sets `is_waking` flag
    /// - Only one thread successfully wakes a worker
    /// - Other threads see the flag and skip wakeup
    #[inline]
    fn wake_one(&self) {
        if self
            .is_waking
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let waker = self
                .lounge
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .wake_one();

            // Reduce the time that occupied lock
            if let Some(waker) = waker {
                waker.wake();
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Worker Implementation

impl Worker {
    #[inline(never)]
    fn steal_global(src: &ListQueue<Runnable>, dst: &ArrayQueue<Runnable>) {
        let mut deque = ArrayDeque::<Runnable, WORKER_QUEUE_SIZE>::new();

        let mut guard = src.lock_pop();
        for _ in 0..WORKER_QUEUE_SIZE {
            if let Some(runnable) = src.pop_with_lock(&mut guard) {
                // SAFETY: WORKER_QUEUE_SIZE == capacity.
                unsafe{ deque.push_back_unchecked(runnable); }
            } else {
                break;
            }
        }
        ::core::mem::drop(guard);

        while let Some(runnable) = deque.pop_front() {
            let ret = dst.push(runnable);
            debug_assert!(ret.is_ok());
            unsafe { ret.unwrap_unchecked(); }
        }
    }

    #[inline(never)]
    fn steal_global_for_work(&self) -> Option<Runnable> {
        let src: &ListQueue<Runnable> = &self.state().queue;
        let dst: &ArrayQueue<Runnable> = self.queue();

        if let Some(r) = src.pop() {
            Worker::steal_global(src, dst);
            self.wake();
            self.wake_one();
            return Some(r);
        }

        None
    }

    #[inline(never)]
    fn steal_global_for_main(&self, state: &State) -> Option<Runnable> {
        let worker_num = state.seats.len() - 1;
        let src: &ListQueue<Runnable> = &state.queue;
        let dst: &ArrayQueue<Runnable> = &state.seats[worker_num].queue;

        if let Some(r) = src.pop() {
            Worker::steal_global(src, dst);
            state.wake_one();
            return Some(r);
        }

        None
    }

    #[inline(never)]
    fn steal_woker(src: &ArrayQueue<Runnable>, dst: &ArrayQueue<Runnable>) {
        let len = src.len() >> 1;
        // if src.len == 1, we do not steal,
        // because we already stealed one before this function. 
        for _ in 0..len {
            if let Some(runnable) = src.pop() {
                let ret = dst.push(runnable);
                debug_assert!(ret.is_ok());
                unsafe { ret.unwrap_unchecked(); }
            } else {
                return;
            }
        }
    }

    #[inline(never)]
    fn steal_worker_for_work(&self) -> Option<Runnable> {
        let state = self.state();
        let dst: &ArrayQueue<Runnable> = self.queue();

        // Pick a random starting point in the iterator list and rotate the list.
        let worker_num = state.seats.len();
        let start = self.xor_shift.next_usize(worker_num);
        let iter = state.seats[start..]
            .iter()
            .chain(state.seats[..start].iter())
            .filter(|seat| !ptr::eq(&seat.queue, dst));

        // Try stealing from each local queue in the list.
        for seat in iter {
            let src: &ArrayQueue<Runnable> = &seat.queue;
            if let Some(r) = src.pop() {
                Worker::steal_woker(src, dst);
                self.wake();
                self.wake_one();
                return Some(r);
            }
        }

        None
    }

    #[inline(never)]
    fn steal_worker_for_main(&self, state: &State) -> Option<Runnable> {
        let worker_num = state.seats.len();
        let dst: &ArrayQueue<Runnable> = &state.seats[worker_num - 1].queue;

        // Pick a random starting point in the iterator list and rotate the list.
        let start = self.xor_shift.next_usize(worker_num);
        let iter = state.seats[start..]
            .iter()
            .chain(state.seats[..start].iter())
            .filter(|seat| !ptr::eq(&seat.queue, dst));

        // Try stealing from each local queue in the list.
        for seat in iter {
            let src: &ArrayQueue<Runnable> = &seat.queue;
            if let Some(r) = src.pop() {
                Worker::steal_woker(src, dst);
                state.wake_one();
                return Some(r);
            }
        }

        None
    }

    /// Returns a reference to the bound executor state
    /// 
    /// # Safety
    /// Must only be called after successful `bind()`
    #[inline(always)]
    const fn state(&self) -> &State {
        debug_assert!(!self.state.get().is_null());
        unsafe{ &*self.state.get() }
    }

    /// Returns a reference to this worker's local queue
    /// 
    /// # Safety
    /// Must only be called after successful `bind()`
    #[inline(always)]
    const fn queue(&self) -> &ArrayQueue<Runnable> {
        debug_assert!(!self.queue.get().is_null());
        unsafe{ &*self.queue.get() }
    }

    /// Transitions worker to sleeping state, registering a waker
    /// 
    /// Returns `true` if this is a new sleep (state changed),
    /// `false` if already sleeping (just updating waker).
    fn sleep(&self, waker: &Waker) -> bool {
        let state = self.state();
        let mut lounge = state.lounge
            .lock()
            .unwrap_or_else(PoisonError::into_inner);

        if self.working.get() {
            // Working → Sleeping
            lounge.insert(self.seat_index.get(), waker);
            self.working.set(false);
            // loop again
        } else {
            // Already not working, update waker
            if !lounge.update(self.seat_index.get(), waker) {
                // Sleeping -> Sleeping
                return false;
            }
            // else: Waking -> Sleeping, loop again
        }

        state.is_waking.store(lounge.is_waking(), Ordering::Release);
        // Working/Waking -> sleeping, try steal again
        true
    }

    /// Wakes this worker (**Sleeping/Waking** → **Working**).
    #[inline]
    fn wake(&self) {
        /// Wakes this worker (Sleeping → Working or Waking → Working).
        #[cold]
        #[inline(never)]
        fn wake_internal(this: &Worker) {
            // debug_assert!( !self.working.get() );
            let state = this.state();
            let mut lounge = state.lounge
                .lock()
                .unwrap_or_else(PoisonError::into_inner);

            lounge.remove(this.seat_index.get());

            state.is_waking.store(lounge.is_waking(), Ordering::Release);

            this.working.set(true);
        }

        if !self.working.get() {
            wake_internal(self);
        }
    }
    
    /// Wakes an other worker if exist. (**Sleeping** → **Waking**).
    #[inline]
    fn wake_one(&self) {
        self.state().wake_one();
    }

    /// Attempts to get a runnable task using the work-stealing hierarchy
    /// 
    /// Priority order (classic work-stealing algorithm):
    /// 1. Local queue (fast path, no synchronization)
    /// 2. Global queue (shared, requires synchronization)
    /// 3. Other workers' queues (work stealing, random victim selection)
    /// 
    /// Returns `Some(Runnable)` if a task was found, `None` otherwise.
    #[inline(always)]
    fn fetch_runnable(&self) -> Option<Runnable> {
        let local_queue = self.queue();
        if let Some(runnable) = local_queue.pop() {
            self.wake();
            return Some(runnable);
        }

        voker_utils::cold_path();
        self.steal_global_for_work().or_else(|| self.steal_worker_for_work())
    }

    /// Attempts to get a runnable task
    /// 
    /// - Return Ready and set **Working** if successed.
    /// - Return Pending and set **Sleeping** if repeatedly failed.
    async fn runnable(&self) -> Runnable {
        poll_fn(|cx| {
            loop {
                if let Some(r) = self.fetch_runnable() {
                    return Poll::Ready(r);
                }
                // Only enter sleep after the second `None`.
                if !self.sleep(cx.waker()) {
                    // Sleeping -> Sleeping, return Pending
                    return Poll::Pending;
                }
                // else: Working/Waking -> Sleeping, try again.
            }
        })
        .await
    }

    /// Worker thread:
    /// - Uses work-stealing from local/global/other workers
    /// - Processes in batches of `RUN_BATCH` tasks before yielding
    async fn work_run(&self) -> ! {
        /// Number of tasks processed before a worker yields to the scheduler.
        /// This prevents long-running tasks from starving other work.
        const RUN_BATCH: usize = 200;

        loop {
            for _ in 0..RUN_BATCH {
                let runnable = self.runnable().await;
                runnable.run();
            }
            futures_lite::future::yield_now().await;
        }
    }


    /// Main thread:
    /// - Polls the last local queue and global queue, support stealing.
    /// - Yields frequently to avoid starving bound workers
    async fn main_run(&self, state: &State) -> ! {
        let last = state.seats.len() - 1;
        let queue = &state.seats[last].queue;

        loop {
            let runnable = queue.pop()
                .or_else(|| self.steal_global_for_main(state))
                .or_else(|| self.steal_worker_for_main(state));

            if let Some(runnable) = runnable {
                runnable.run();
            }
            futures_lite::future::yield_now().await;
        }
    }

    /// Main worker execution loop
    /// 
    /// Continuously processes tasks until `stop_signal` future completes.
    async fn run<T>(&self, state: &State, stop_signal: impl Future<Output = T>) -> T {
        let run_forever = async {
            if self.queue.get().is_null() {
                self.main_run(state).await;
            } else {
                self.work_run().await;
            }
        };

        // Run until stop signal completes
        run_forever.or(stop_signal).await
    }
}

// -----------------------------------------------------------------------------
// GlobalExecutor Implementation

impl<'a> GlobalExecutor<'a> {
    /// Creates a new executor with the specified number of worker seats
    /// 
    /// # Arguments
    /// - `num` - Number of worker threads this executor will support
    /// 
    /// # Initial State
    /// - Global queue is empty
    /// - All seats are unoccupied
    /// - Lounge has no sleeping workers
    /// - `is_waking` is true (all workers considered active initially)
    pub fn new(worker_num: usize) -> Self {
        // idle capacity is 32 * 64 == 2048, appropriate?
        let queue: ListQueue<Runnable> = ListQueue::new(32);
        // [0..worker_num] for worker thread, [worker_num] for main thread.
        let seats: CachePadded<Box<[Seat]>> = CachePadded::new(
            (0..=worker_num).map(|_|Seat{
                occupied: AtomicBool::new(false),
                queue: ArrayQueue::new(WORKER_QUEUE_SIZE),
            }).collect()
        );
        // occupy main thread queue
        seats[worker_num].occupied.store(true, Ordering::Release);
        // [0..worker_num] for worker thread, without main thread
        let lounge: Mutex<Lounge> = Mutex::new(Lounge {
            waking: 0,
            sleeping: 0,
            wakers: (0..worker_num).map(|_|None).collect(),
        });
        let is_waking: AtomicBool = AtomicBool::new(true);

        Self {
            state: State { queue, seats, lounge, is_waking },
            _marker: PhantomData,
        }
    }

    /// Binds this worker to a specific executor, claiming a seat.
    /// 
    /// This is called when a thread joins a task pool. The worker
    /// atomically claims an unoccupied seat and stores pointers to
    /// the executor state and local queue.
    /// 
    /// # Safety
    /// Worker internally retains pointers to the `GlobalExecutor` field.
    /// To ensure its long-term validity, `GlobalExecutor` typically require
    /// the use of smart pointer wrapping.
    pub fn bind_local_worker(&self) {
        LOCAL_WORKER.with(|worker|{
            if !worker.state.get().is_null() {
                return;
            }

            worker.state.set(&self.state);

            for (index, seat) in self.state.seats.iter().enumerate()  {
                if !seat.occupied.swap(true, Ordering::AcqRel) {
                    worker.queue.set(&seat.queue);
                    worker.seat_index.set(index);
                    worker.xor_shift.randomize();
                    return;
                }
            }

            unreachable!("Failed to bind worker: No available seats in executor");
        })
    }

    /// Spawns a future onto the executor's global queue
    /// 
    /// The task will be automatically scheduled and executed by worker threads.
    /// Returns a `Task` handle that can be used to await the result.
    pub fn spawn<T: Send + 'a>(&self, future: impl Future<Output = T> + Send + 'a) -> Task<T> {
        let state = &self.state;

        let schedule = move |runnable| {
            state.queue.push(runnable);
            state.wake_one();
        };

        // # SAFETY: See in async_task::spawn_unchecked
        let (runnable, task) = unsafe {
            async_task::Builder::new()
                .propagate_panic(true)
                .spawn_unchecked(|()|future, schedule)
        };

        // Immediately schedule the task for execution
        runnable.schedule();

        task
    }

    /// Runs the executor until the given future completes
    #[inline]
    pub async fn run<T>(&self, stop_signal: impl Future<Output = T>) -> T {
        LOCAL_WORKER.with(|local_worker: &Worker|{
            // SAFETY: The thread-local worker lives as long as the thread.
            let worker: &'static Worker = unsafe{ core::mem::transmute(local_worker) };
            worker.run(&self.state, stop_signal)
        }).await
    }
}

unsafe impl Send for GlobalExecutor<'_> {}
unsafe impl Sync for GlobalExecutor<'_> {}
impl UnwindSafe for GlobalExecutor<'_> {}
impl RefUnwindSafe for GlobalExecutor<'_> {}

impl fmt::Debug for GlobalExecutor<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let worker_num = self.state.seats.len() - 1;
        f.debug_struct("GlobalExecutor")
            .field("worker_num", &worker_num)
            .finish()
    }
}
