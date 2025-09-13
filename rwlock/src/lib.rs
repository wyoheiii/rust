use std::{cell::UnsafeCell, ops::{Deref, DerefMut}, sync::atomic::AtomicU32, u32};
use std::sync::atomic::Ordering::{Acquire, Release, Relaxed};

use atomic_wait::{wait, wake_all, wake_one};

pub struct RwLock<T> {
	// readers count (0..=u32::MAX-1) or writer locked (u32::MAX)
	// 2 * wait reader + wait writer ? 1:0 
	state: AtomicU32,
	writer_wake_counter: AtomicU32,
	value: UnsafeCell<T>,
}

unsafe impl<T> Sync for RwLock<T> where T: Send + Sync {}

impl<T> RwLock<T> {
	pub const fn new(value: T) -> Self {
		Self {
			state: AtomicU32::new(0), //unlocked
			writer_wake_counter: AtomicU32::new(0),
			value: UnsafeCell::new(value),
		}
	}

	pub fn read(&self) -> ReadGuard<T> {
		let mut s = self.state.load( Relaxed);

		loop {
			if s % 2 == 0 {
				assert!( s != u32::MAX - 2, "too many readers");

				match self.state.
				compare_exchange_weak(s, s + 2 , Acquire, Relaxed) {
					Ok(_) => return ReadGuard { rwlock: self},
					Err(e) => s = e,
				}
			}
			if s % 2 == 1 {
					wait(&self.state, s);
					s = self.state.load(Relaxed);
				}
			}
		}

	pub fn write(&self) -> WriteGuard<T> {
		let mut s = self.state.load(Relaxed);
		
		loop {
			if s <= 1 {
				match self.state.compare_exchange(s, u32::MAX, Acquire, Relaxed) {
					Ok(_) => return WriteGuard { rwlock: self },
					Err(e) => { s = e; continue; }
				}
			}

			if s % 2 == 0 {
				match self.state.compare_exchange(s, s + 1, Relaxed, Relaxed) {
					Ok(_) => {}
					Err(e) => { s = e; continue; }
				}
			}

			let w = self.writer_wake_counter.load(Acquire);
			s = self.state.load(Relaxed);

			if s >= 2 {
				wait(&self.writer_wake_counter, w);
				s = self.state.load(Relaxed);
			} 
		}
	}
}

pub struct ReadGuard<'a, T> {
	rwlock: &'a RwLock<T>,
}

impl<T> Drop for ReadGuard<'_, T> {
	fn drop(&mut self) {
		if self.rwlock.state.fetch_sub(2, Release) == 3 {
			// 3->1 writer wait
			self.rwlock.writer_wake_counter.fetch_add(1,Release);
			wake_one(&self.rwlock.writer_wake_counter);
		}
	}
}


impl<T> Deref for ReadGuard<'_, T> {
	type Target = T;
	fn deref(&self) -> &T {
		unsafe { &*self.rwlock.value.get() }
	}
}

pub struct WriteGuard<'a, T> {
	rwlock: &'a RwLock<T>,
}

impl<T> Drop for WriteGuard<'_, T> {
	fn drop(&mut self) {
		self.rwlock.state.store(0, Release);
		self.rwlock.writer_wake_counter.fetch_add(1, Release);
		wake_one(&self.rwlock.writer_wake_counter);
		wake_all(&self.rwlock.state);
	}
}

impl<T> Deref for WriteGuard<'_, T> {
	type Target = T;
	
	fn deref(&self) -> &Self::Target {
		unsafe { &*self.rwlock.value.get() }
	}
}

impl<T> DerefMut for WriteGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut T {
		unsafe { &mut *self.rwlock.value.get() }
	}
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
  
    }
}
