use std::{cell::UnsafeCell, ops::{Deref, DerefMut}, sync::atomic::AtomicU32};
use std::sync::atomic::Ordering::{Acquire, Release, Relaxed};

use atomic_wait::{wait, wake_one};
pub struct Mutex<T> {
  /// 0 = unlocked, 1 = locked, 2 = locked with waiters
  state: AtomicU32,
  value: UnsafeCell<T>,
}
unsafe impl<T> Sync for Mutex<T> where T: Send {}

impl<T> Mutex<T> {
  pub fn new(value: T) -> Self {
    Self {
      state: AtomicU32::new(0),
      value: UnsafeCell::new(value),
    }
  }

  pub fn lock(&self)-> MutexGuard<T> {
    if self.state.compare_exchange(0, 1, Acquire, Relaxed).is_err() {
      lock_contended(&self.state);
    }

    MutexGuard { mutex: self }
  }
  
}

fn lock_contended(state: &AtomicU32) {
  let mut spin_count = 0;

  while state.load(Relaxed) == 1 && spin_count < 100 {
    spin_count += 1;
    std::hint::spin_loop();
  }

  if state.compare_exchange(0, 1, Acquire, Relaxed).is_ok() {
    return;
  }

  while state.swap(2, Acquire) != 0 {
    wait(state, 2);
  }
}

pub struct MutexGuard<'a, T> {
  mutex: &'a Mutex<T>,
}

unsafe impl<T> Send for MutexGuard<'_, T> where T: Send {}

impl<T> Deref for MutexGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    unsafe { &*self.mutex.value.get() }
  }
}

impl<T> DerefMut for MutexGuard<'_, T> {
  fn deref_mut(&mut self)-> &mut T {
    unsafe { &mut *self.mutex.value.get() }
  }
}

impl<T> Drop for MutexGuard<'_, T> {
  fn drop(&mut self) {
    if self.mutex.state.swap(0, Release) == 2 {
      wake_one(&self.mutex.state);
    }
  }
}
 
#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_mutex() {
    let m = Mutex::new(0);
    std::hint::black_box(&m);
    let start = std::time::Instant::now();
    for _ in 0..500000 {
      *m.lock() += 1;
    }

    let duration = start.elapsed();
    println!("locked {} times in {:?}", *m.lock(), duration);
    assert!(*m.lock() == 500000);
  }
}
