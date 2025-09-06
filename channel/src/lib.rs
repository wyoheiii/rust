use std::marker::PhantomData;
use std::{cell::UnsafeCell, mem::MaybeUninit, sync::atomic::AtomicBool, thread::Thread};
use std::sync::atomic::Ordering::{Release, Relaxed, Acquire};
use std::thread;
pub struct Channel<T> {
  // maybeuniitはoptionのunsafe版
  message: UnsafeCell<MaybeUninit<T>>,
  ready: AtomicBool,
}

unsafe impl <T> Sync for Channel<T> where T: Send {}

impl<T> Channel<T> {
  pub const fn new() -> Self {
    Channel {
      message: UnsafeCell::new(MaybeUninit::uninit()),
      ready: AtomicBool::new(false),
    }
  }

  // 同じスコープで一つのチャネルしか使えないことを保証するために、&mut selfを取る
  pub fn split(&mut self) -> (Sender<T>, Receiver<T>) {
    //　送信されなかった古いメッセージをdropし、readyをfalseに戻す
    *self = Self::new();
    (Sender {
      channel: self,
      receiving_thread: thread::current(),
    }, Receiver {
      channel: self,
      _no_send: PhantomData,
    })
  }
}

impl<T> Drop for Channel<T> {
  // get_mutは唯一の参照を持っているときにしか呼び出せないため、排他アクセスの保証がある
  fn drop(&mut self) {
    if *self.ready.get_mut() {
      unsafe { (*self.message.get()).assume_init_drop(); }
    }
  }
}

pub struct Sender<'a, T> {
  channel: &'a Channel<T>,
  receiving_thread: Thread,
}

impl<'a, T> Sender<'a, T> {
  pub fn send(self, value: T) {
    unsafe { (*self.channel.message.get()).write(value); }
    self.channel.ready.store(true, Release);
    self.receiving_thread.unpark();
  }
}

pub struct Receiver<'a, T> {
  channel: &'a Channel<T>,
  // receiverが別のスレッドで使われることを防ぐ.*const ()はSendトレイトを実装しないため
  _no_send: PhantomData<*const ()>,
}

impl<'a, T> Receiver<'a, T> {
  pub fn is_ready(&self) -> bool {
    self.channel.ready.load(Relaxed)
  }

  pub fn receive(self)-> T {
    // sender以外のunparkでスレッドが起きることを防ぐためのループ
    if !self.channel.ready.swap(false, Acquire) {
      thread::park();
    }
    unsafe { (*self.channel.message.get()).assume_init_read() }
  }
}




#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    #[test]
    fn it_works() {
      let mut channel = Channel::new();
      thread::scope(|s| {
        let (sender, receiver) = channel.split();
        let t = thread::current();
        s.spawn(move || {
          sender.send(42);
          t.unpark();
        });
        assert_eq!(receiver.receive(), 42);
      });

    }
}
