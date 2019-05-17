use crate::subject::{Subject, CallbackPtr};

pub struct Subscription<'a, T> {
  source: Subject<'a, T>,
  callback: CallbackPtr<'a, T>,
}

impl<'a, T:'a> Subscription<'a, T> {
  pub fn new(source: Subject<'a, T>, ptr: CallbackPtr<'a, T>)-> Self {
    Self {
      source, callback: ptr
    }
  }

  pub fn unsubscribe(mut self) {
   self.source.remove_callback(self.callback);
  }
}