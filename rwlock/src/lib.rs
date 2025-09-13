use std::{cell::UnsafeCell, ops::{Deref, DerefMut}, sync::atomic::AtomicU32};

pub struct RwLock<T> {
	state: AtomicU32,
	value: UnsafeCell<T>,
}

unsafe impl<T> Sync for RwLock<T> where T: Send + Sync {}

impl<T> RwLock<T> {
	pub const fn new(value: T) -> Self {
		Self {
			state: AtomicU32::new(0), //unlocked
			value: UnsafeCell::new(value),
		}
	}
}

pub struct ReadGuard<'a, T> {
	rwlock: &'a RwLock<T>,
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
