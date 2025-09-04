use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::{Acquire, Release};

pub struct SpinLock<T> {
  locked:AtomicBool,
  value: UnsafeCell<T>,
}

// Tに対して１つのスレッドがアクセスすることを保証する
unsafe impl<T> Sync for SpinLock<T> where T:Send {}

impl<T> SpinLock<T> {
  pub const fn new(value: T) -> Self {
    Self {
      locked: AtomicBool::new(false),
      value: UnsafeCell::new(value),
    }
  }

  pub fn lock(&self) -> Guard<T> {
    while self.locked.swap(true, Acquire) {
      std::hint::spin_loop();
    }

    Guard { lock: self }
  }

  pub fn unlock(&self) {
    self.locked.store(false, Release);
  }
}

// Guardが存在することでlockされてることを保証する
pub struct Guard<'a, T> {
  lock: &'a SpinLock<T>,
}

unsafe impl<T> Send for Guard<'_, T> where T: Send {}
unsafe impl<T> Sync for Guard<'_, T> where T: Sync {}

impl<T> Deref for Guard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    unsafe { &*self.lock.value.get() }
  }
}

impl<T> DerefMut for Guard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe { &mut *self.lock.value.get() }
  }
}

impl<T> Drop for Guard<'_, T> {
  fn drop(&mut self) {
    self.lock.locked.store(false, Release);
  }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    #[test]
    fn test_spinlock() {
      let l = SpinLock::new(0);
      thread::scope(|s| {
        for _ in 0..10 {
          s.spawn(|| {
            for _ in 0..100 {
              *l.lock() += 1;
            }
          });
        }
      });
      let g = l.lock();
      assert_eq!(*g, 1000);
    }

}
