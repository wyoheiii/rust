use std::cell::UnsafeCell;
use std::mem::ManuallyDrop;
use std::sync::atomic::fence;
use std::usize;
use std::{ops::Deref, ptr::NonNull, sync::atomic::AtomicUsize};
use std::sync::atomic::Ordering::{Relaxed, Release, Acquire};

struct ArcData<T> {
  // Arc
  data_ref_count: AtomicUsize,
  // weakの数。arcが１つでもあれば+1
  alloc_ref_count: AtomicUsize,
  // weakしか残ってなければdropされる
  data: UnsafeCell<ManuallyDrop<T>>,
}

pub struct Weak<T> {
  ptr: NonNull<ArcData<T>>,
}

unsafe impl<T: Send + Sync> Send for Weak<T> {}
unsafe impl<T: Send + Sync> Sync for Weak<T> {}

impl<T> Weak<T> {

  fn data(&self)-> &ArcData<T> {
    unsafe{ self.ptr.as_ref()}
  }

  pub fn upgrade(&self) -> Option<Arc<T>> {
    let mut count = self.data().data_ref_count.load(Relaxed);
    loop {
      if count == 0 {
        return None;
      }
      assert!(count < usize::MAX);
      if let Err(e) = self.data().data_ref_count.compare_exchange_weak(
        count,
        count + 1,
        Relaxed,
        Relaxed,
      ) {
        count = e;
        continue;
      }
      return Some(Arc { ptr: self.ptr });
    }
  }
}

impl<T> Clone for Weak<T> {
  fn clone(&self) -> Self {
    if self.data().alloc_ref_count.fetch_add(1, Relaxed) > usize::MAX / 2{
      std::process::abort();
    }

    Weak { ptr: self.ptr }
  }
}

impl<T> Drop for Weak<T> {
  fn drop(&mut self) {
    if self.data().alloc_ref_count.fetch_sub(1, Release) == 1 {
      fence(Acquire);
      // 最後の参照がドロップされたとき、メモリを解放する
      drop(unsafe { Box::from_raw(self.ptr.as_ptr()) });
    }
  }
}


pub struct Arc<T> {
  ptr: NonNull<ArcData<T>>,
}

unsafe impl<T: Send + Sync> Send for Arc<T> {}
unsafe impl<T: Send + Sync> Sync for Arc<T> {}

impl<T> Arc<T> {
  pub fn new(data: T) -> Arc<T> {
    Arc {
      ptr: NonNull::from(
        // leakを使うことで排他所有権を放棄
          Box::leak(Box::new(ArcData {
          data_ref_count: AtomicUsize::new(1),
          alloc_ref_count: AtomicUsize::new(1),
          data: UnsafeCell::new(ManuallyDrop::new(data)),
      }))),
    }
  }

  fn data(&self) -> &ArcData<T> {
    unsafe { self.ptr.as_ref() }
  }

  pub fn get_mut(arc: &mut Self)-> Option<&mut T> {
    if arc.data().
    alloc_ref_count.
    compare_exchange(1,usize::MAX, Acquire, Relaxed).is_err() {
      return None;
    }

    let is_unique = arc.data().data_ref_count.load(Relaxed) == 1;
    arc.data().alloc_ref_count.store(1, Release);
    if !is_unique {
      return None;
    }

    fence(Acquire);
    unsafe { Some(&mut *arc.data().data.get()) }
  }

  pub fn downgrade(arc: &Self) -> Weak<T> {
    let mut n = arc.data().alloc_ref_count.load(Relaxed);
    loop {
      if n == usize::MAX {
        std::hint::spin_loop();
        n = arc.data().alloc_ref_count.load(Relaxed);
        continue;
      }
      assert!(n < usize::MAX - 1);
      if let Err(e) = arc.data().alloc_ref_count.compare_exchange_weak(
        n,
        n + 1,
        Acquire,
        Relaxed,
      ) {
        n = e;
        continue;
      }
      return Weak { ptr: arc.ptr };
    }
  }
}

impl<T> Deref for Arc<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    unsafe { &*self.data().data.get() }
  }
}

impl<T> Clone for Arc<T> {
  fn clone(&self) -> Arc<T> {
    if self.data().data_ref_count.fetch_add(1, Relaxed) > usize::MAX / 2{
      std::process::abort();
    }

    Arc { ptr: self.ptr }
  }
}

impl<T> Drop for Arc<T> {
  fn drop(&mut self) {
    // fetch_subでloadを行ってるからfenceで先行発生関係ができる
    if self.data().data_ref_count.fetch_sub(1, Release) == 1 {
      fence(Acquire);
      // 最後の参照がドロップされたとき、メモリを解放する
      unsafe {ManuallyDrop::drop(&mut *self.data().data.get())};
      // 暗黙のweakのドロップ
      drop(Weak { ptr: self.ptr } );
    }
  }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
      static NUM_DROPS: AtomicUsize = AtomicUsize::new(0);

      struct DetectDrop;

      impl Drop for DetectDrop {
        fn drop(&mut self) {
          NUM_DROPS.fetch_add(1, Relaxed);
        }
      }

      // Create an Arc with two weak pointers.
      let mut x = Arc::new(("hello", DetectDrop));
      let y = Arc::downgrade(&x);
      let z = Arc::downgrade(&x);
      let t = std::thread::spawn(move || {
        // Weak pointer should be upgradable at this point.
        let y = y.upgrade().unwrap();
        assert_eq!(y.0, "hello");
      });

      assert_eq!(x.0, "hello");
      Arc::get_mut(&mut x);
      t.join().unwrap();

      // The data shouldn't be dropped yet,
      // and the weak pointer should be upgradable.
      assert_eq!(NUM_DROPS.load(Relaxed), 0);
      assert!(z.upgrade().is_some());

      drop(x);

      // Now, the data should be dropped, and the
      // weak pointer should no longer be upgradable.
      assert_eq!(NUM_DROPS.load(Relaxed), 1);
      assert!(z.upgrade().is_none());
    }

}
