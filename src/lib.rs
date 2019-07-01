#![feature(external_doc)]
#![doc(include = "../README.md")]
#![feature(drain_filter)]

pub mod observable;
pub mod ops;
pub mod subject;
pub mod subscribable;
pub mod subscriber;
pub mod subscription;

pub mod prelude {
  pub use crate::observable;
  pub use crate::observable::Observable;
  pub use crate::ops;
  pub use crate::subject;
  pub use crate::subject::Subject;
  pub use crate::subscribable;
  pub use crate::subscribable::*;
  pub use crate::subscriber;
  pub use crate::subscriber::Subscriber;
  pub use crate::subscription;
  pub use crate::subscription::*;
  pub use crate::Observer;
}

pub trait Observer {
  type Item;
  type Err;

  fn next(&self, v: &Self::Item) -> &Self;

  fn complete(&mut self);

  fn error(&mut self, err: &Self::Err);
}
