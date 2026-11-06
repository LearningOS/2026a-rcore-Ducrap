/*!
Synchronization, interior mutability, and deadlock resource accounting.
Assistance: Codex; reference: rCore Guide, chapter 8 exercise.
*/

mod condvar;
mod deadlock;
mod mutex;
mod semaphore;
mod up;

pub use condvar::Condvar;
pub use deadlock::ResourceSet;
pub use mutex::{Mutex, MutexBlocking, MutexSpin};
pub use semaphore::Semaphore;
pub use up::UPSafeCell;
