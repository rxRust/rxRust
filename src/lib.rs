#![feature(external_doc)]
#![doc(include = "../README.md")]

pub mod error;
pub mod observable;
pub mod ops;
pub mod subject;
pub mod subscription;

pub mod prelude {
  pub use crate::error::{
    NextWhitoutError, NextWithError, WithErr, WithErrByRef,
  };
  pub use crate::observable::*;
  pub use crate::subject::Subject;
  pub use crate::subscription::*;
  pub use crate::Observer;
}

pub trait Observer {
  type Item;
  type Err;

  fn next(&self, v: Self::Item) -> &Self;

  fn complete(self);

  fn error(self, err: Self::Err);
}
