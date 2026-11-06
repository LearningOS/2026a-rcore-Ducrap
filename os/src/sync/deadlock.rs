/* Detect unresolved resource requests with the course's Work/Finish algorithm. */

use alloc::{vec, vec::Vec};

/// Resource accounting is maintained even while rejection is disabled.
pub struct ResourceSet {
    available: Vec<usize>,
    allocation: Vec<Vec<usize>>,
    need: Vec<Vec<usize>>,
    alive: Vec<bool>,
}

impl ResourceSet {
    /// Create an empty resource family.
    pub fn new() -> Self {
        Self {
            available: Vec::new(),
            allocation: Vec::new(),
            need: Vec::new(),
            alive: Vec::new(),
        }
    }

    /// Add an available resource column to every thread row.
    pub fn add_resource(&mut self, id: usize, capacity: usize) {
        let columns = self.available.len().max(id + 1);
        self.available.resize(columns, 0);
        self.available[id] = capacity;
        for row in self.allocation.iter_mut().chain(self.need.iter_mut()) {
            row.resize(columns, 0);
            row[id] = 0;
        }
    }

    fn ensure_thread(&mut self, tid: usize) {
        while self.allocation.len() <= tid {
            self.allocation.push(vec![0; self.available.len()]);
            self.need.push(vec![0; self.available.len()]);
            self.alive.push(true);
        }
    }

    /// Start accounting for a newly assigned thread ID.
    pub fn reset_thread(&mut self, tid: usize) {
        self.ensure_thread(tid);
        self.allocation[tid].fill(0);
        self.need[tid].fill(0);
        self.alive[tid] = true;
    }

    /// Retire a thread without pretending its held primitives were released.
    pub fn remove_thread(&mut self, tid: usize) {
        if tid < self.alive.len() {
            self.alive[tid] = false;
            self.need[tid].fill(0);
            // Exiting does not unlock the actual primitives. Its held units stay
            // unavailable, including when this thread ID is subsequently reused.
            self.allocation[tid].fill(0);
        }
    }

    /// Record an outstanding acquisition, rolling it back if it is unsafe.
    pub fn request(&mut self, tid: usize, id: usize, detect: bool) -> bool {
        self.ensure_thread(tid);
        self.need[tid][id] += 1;
        if detect && !self.is_safe() {
            self.need[tid][id] -= 1;
            return false;
        }
        true
    }

    /// Convert a completed acquisition from Need into Allocation.
    pub fn acquired(&mut self, tid: usize, id: usize) {
        self.need[tid][id] -= 1;
        self.allocation[tid][id] += 1;
        self.available[id] -= 1;
    }

    /// Return a held resource, or produce a new unowned semaphore token.
    pub fn released(&mut self, tid: usize, id: usize) {
        self.ensure_thread(tid);
        // A semaphore may be posted by a thread that never acquired a token.
        if self.allocation[tid][id] > 0 {
            self.allocation[tid][id] -= 1;
        }
        self.available[id] += 1;
    }

    /// Need records outstanding calls, not an advance declaration of maximum use.
    /// Reference: https://learningos.cn/rCore-Tutorial-Guide/chapter8/5exercise.html
    pub fn is_safe(&self) -> bool {
        let mut work = self.available.clone();
        let mut finish: Vec<bool> = self.alive.iter().map(|alive| !alive).collect();
        loop {
            let mut progress = false;
            for tid in 0..finish.len() {
                if !finish[tid]
                    && self.need[tid]
                        .iter()
                        .zip(&work)
                        .all(|(need, free)| need <= free)
                {
                    for (free, held) in work.iter_mut().zip(&self.allocation[tid]) {
                        *free += held;
                    }
                    finish[tid] = true;
                    progress = true;
                }
            }
            if !progress {
                return finish.iter().all(|done| *done);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acquire(set: &mut ResourceSet, tid: usize, id: usize) {
        assert!(set.request(tid, id, true));
        set.acquired(tid, id);
    }

    #[test]
    fn recursive_mutex_is_rejected_without_changing_the_owner() {
        let mut set = ResourceSet::new();
        set.add_resource(0, 1);
        acquire(&mut set, 0, 0);
        assert!(!set.request(0, 0, true));
        assert_eq!(set.need[0][0], 0);
        assert_eq!(set.allocation[0][0], 1);
        set.released(0, 0);
        acquire(&mut set, 0, 0);
    }

    #[test]
    fn waiting_for_a_runnable_owner_is_safe() {
        let mut set = ResourceSet::new();
        set.add_resource(0, 1);
        acquire(&mut set, 0, 0);
        assert!(set.request(1, 0, true));
        set.released(0, 0);
        set.acquired(1, 0);
        assert!(set.is_safe());
    }

    #[test]
    fn circular_wait_is_rejected_and_rolled_back() {
        let mut set = ResourceSet::new();
        set.add_resource(0, 1);
        set.add_resource(1, 1);
        acquire(&mut set, 0, 0);
        acquire(&mut set, 1, 1);
        assert!(set.request(0, 1, true));
        assert!(!set.request(1, 0, true));
        assert_eq!(set.need[1][0], 0);
        set.released(1, 1);
        set.acquired(0, 1);
        assert!(set.is_safe());
    }

    #[test]
    fn multiple_units_and_a_future_release_allow_progress() {
        let mut set = ResourceSet::new();
        set.add_resource(0, 2);
        set.add_resource(1, 2);
        acquire(&mut set, 0, 0);
        acquire(&mut set, 1, 0);
        acquire(&mut set, 2, 1);
        acquire(&mut set, 3, 1);
        assert!(set.request(0, 1, true));
        assert!(set.request(2, 0, true));
    }

    #[test]
    fn accounting_survives_disabled_detection_and_resource_growth() {
        let mut set = ResourceSet::new();
        set.add_resource(0, 1);
        assert!(set.request(0, 0, false));
        set.acquired(0, 0);
        set.add_resource(1, 2);
        assert!(!set.request(0, 0, true));
        acquire(&mut set, 0, 1);
    }

    #[test]
    fn posts_without_a_prior_acquisition_do_not_underflow() {
        let mut set = ResourceSet::new();
        set.add_resource(0, 0);
        set.released(2, 0);
        acquire(&mut set, 0, 0);
    }

    #[test]
    fn an_exited_owner_cannot_be_simulated_as_releasing_a_lock() {
        let mut set = ResourceSet::new();
        set.add_resource(0, 1);
        acquire(&mut set, 0, 0);
        set.remove_thread(0);
        set.reset_thread(0);
        assert!(!set.request(0, 0, true));
    }

    #[test]
    fn condition_waiter_reacquisition_remains_an_outstanding_mutex_request() {
        let mut set = ResourceSet::new();
        set.add_resource(0, 1);
        set.add_resource(1, 1);
        acquire(&mut set, 0, 0);
        acquire(&mut set, 0, 1);
        // Thread 0 keeps resource 0 while condition-waiting with resource 1.
        set.released(0, 1);
        acquire(&mut set, 1, 1);
        assert!(set.request(0, 1, true));
        assert!(!set.request(1, 0, true));
    }
}
