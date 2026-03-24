use crate::sync::atomic::{AtomicUsize, Ordering};
use crate::time::Duration;
use crate::utils::{ArrayQueue, ListQueue};

use super::error::{RecvError, RecvTimeoutError, SendError, TryRecvError, TrySendError};

pub(super) struct ListChannel<T> {
    queue: ListQueue<T>,
    senders: AtomicUsize,
    receivers: AtomicUsize,
}

impl<T> ListChannel<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            queue: ListQueue::default(),
            senders: AtomicUsize::new(1),
            receivers: AtomicUsize::new(1),
        }
    }

    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        // spin lock always return ok.
        if let Some(val) = self.queue.pop() {
            return Ok(val);
        }
        
        if self.senders.load(Ordering::Acquire) == 0
            && self.queue.is_empty() // Ensure that no new data is pushed
        {
            Err(TryRecvError::Disconnected)
        } else {
            Err(TryRecvError::Empty)
        }
    }

    pub fn recv(&self) -> Result<T, RecvError> {
        let backoff = crate::utils::Backoff::new();
        loop {
            if let Some(val) = self.queue.pop() {
                return Ok(val);
            }

            if self.senders.load(Ordering::Acquire) == 0
                && self.queue.is_empty() // Ensure that no new data is pushed
            {
                return Err(RecvError);
            }
            backoff.snooze();
        }
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvTimeoutError> {
        let backoff = crate::utils::Backoff::new();
        let instant = crate::time::Instant::now();
        loop {
            if let Some(val) = self.queue.pop() {
                return Ok(val);
            }

            if self.senders.load(Ordering::Acquire) == 0
                && self.queue.is_empty() // Ensure that no new data is pushed
            {
                return Err(RecvTimeoutError::Disconnected);
            }

            if instant.elapsed() > timeout {
                return Err(RecvTimeoutError::Timeout);
            }
            backoff.snooze();
        }
    }

    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        // Allow for relaxed receiver judgement.
        if self.receivers.load(Ordering::Relaxed) == 0 {
            return Err(SendError(t));
        }
        self.queue.push(t);
        Ok(())
    }

    #[inline(always)]
    pub fn add_senders(&self) {
        self.senders.fetch_add(1, Ordering::Relaxed);
    }
    #[inline(always)]
    pub fn sub_senders(&self) {
        self.senders.fetch_sub(1, Ordering::Release);
    }
    // #[inline(always)]
    // pub fn add_receivers(&self) {
    //     self.receivers.fetch_add(1, Ordering::Relaxed);
    // }
    #[inline(always)]
    pub fn sub_receivers(&self) {
        self.receivers.fetch_sub(1, Ordering::Relaxed);
    }
}

pub(crate) struct ArrayChannel<T> {
    queue: ArrayQueue<T>,
    senders: AtomicUsize,
    receivers: AtomicUsize,
}

impl<T> ArrayChannel<T> {
    #[inline]
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: ArrayQueue::new(capacity), // panic if capacity == 0
            senders: AtomicUsize::new(1),
            receivers: AtomicUsize::new(1),
        }
    }

    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        // spin lock always return ok.
        if let Some(val) = self.queue.pop() {
            return Ok(val);
        }
        
        if self.senders.load(Ordering::Acquire) == 0 
            && self.queue.is_empty() // Ensure that no new data is pushed
        {
            Err(TryRecvError::Disconnected)
        } else {
            Err(TryRecvError::Empty)
        }
    }

    pub fn recv(&self) -> Result<T, RecvError> {
        let backoff = crate::utils::Backoff::new();
        loop {
            if let Some(val) = self.queue.pop() {
                return Ok(val);
            }

            if self.senders.load(Ordering::Acquire) == 0
                && self.queue.is_empty() // Ensure that no new data is pushed
            {
                return Err(RecvError);
            }
            backoff.snooze();
        }
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvTimeoutError> {
        let backoff = crate::utils::Backoff::new();
        let instant = crate::time::Instant::now();
        loop {
            if let Some(val) = self.queue.pop() {
                return Ok(val);
            }

            if self.senders.load(Ordering::Acquire) == 0
                && self.queue.is_empty() // Ensure that no new data is pushed
            {
                return Err(RecvTimeoutError::Disconnected);
            }

            if instant.elapsed() > timeout {
                return Err(RecvTimeoutError::Timeout);
            }
            backoff.snooze();
        }
    }

    pub fn try_send(&self, t: T) -> Result<(), TrySendError<T>> {
        // Allow for relaxed receiver judgement.
        if self.receivers.load(Ordering::Relaxed) == 0 {
            return Err(TrySendError::Disconnected(t));
        }

        if let Err(val) = self.queue.push(t) {
            Err(TrySendError::Full(val))
        } else {
            Ok(())
        }
    }

    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        let backoff = crate::utils::Backoff::new();
        let mut value = t;
        loop {
            // Allow for relaxed receiver judgement.
            if self.receivers.load(Ordering::Relaxed) == 0 {
                return Err(SendError(value));
            }

            if let Err(val) = self.queue.push(value) {
                value = val;
                backoff.snooze();
            } else {
                return Ok(());
            }
        }
    }

    #[inline(always)]
    pub fn add_senders(&self) {
        self.senders.fetch_add(1, Ordering::Relaxed);
    }
    #[inline(always)]
    pub fn sub_senders(&self) {
        self.senders.fetch_sub(1, Ordering::Release);
    }
    // #[inline(always)]
    // pub fn add_receivers(&self) {
    //     self.receivers.fetch_add(1, Ordering::Relaxed);
    // }
    #[inline(always)]
    pub fn sub_receivers(&self) {
        self.receivers.fetch_sub(1, Ordering::Relaxed);
    }
}
