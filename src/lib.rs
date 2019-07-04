#![feature(external_doc, fn_traits, step_trait, unboxed_closures, drain_filter)]
#![doc(include = "../README.md")]

pub mod function;
pub mod observable;
pub mod ops;
pub mod scheduler;
pub mod subject;
pub mod subscribable;
pub mod subscriber;
pub mod subscription;

pub mod prelude {
  pub use crate::function::*;
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
  pub use ops::{Fork, Multicast};
}

pub trait Observer {
  type Item;
  type Err;

  fn next(&self, v: &Self::Item) -> subscribable::OState<Self::Err>;

  fn complete(&mut self);

  fn error(&mut self, err: &Self::Err);

  fn is_stopped(&self) -> bool;
}
