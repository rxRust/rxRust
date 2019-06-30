#![feature(external_doc)]
#![doc(include = "../README.md")]
#![feature(drain_filter)]

pub mod ops;
pub mod subject;
pub mod subscribable;
pub mod subscriber;
pub mod subscription;

pub mod prelude {
  pub use crate::subject::Subject;
  pub use crate::subscribable::*;
  pub use crate::subscriber::Subscriber;
  pub use crate::subscription::*;
  pub use crate::Observer;
}

pub trait Observer {
  type Item;
  type Err;

  fn next(&self, v: &Self::Item) -> &Self;

  fn complete(self);

  fn error(self, err: &Self::Err);
}
