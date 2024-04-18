//! Reactive extensions library for Rust: a library for
//! [Reactive Programming](http://reactivex.io/) using
//! [Observable](crate::observable::Observable), to make
//! it easier to compose asynchronous or callback-based code.
#![doc = include_str!("../README.md")]

#[cfg(test)]
extern crate float_cmp;

#[cfg(test)]
#[macro_use]
extern crate bencher;

pub mod behavior;
pub mod observable;
pub mod observer;
pub mod ops;
pub mod rc;
pub mod scheduler;
pub mod subject;
pub mod subscriber;
pub mod subscription;
pub mod type_hint;

pub mod prelude {

  pub use crate::behavior::*;
  pub use crate::observable;
  pub use crate::observable::*;
  pub use crate::observer;
  pub use crate::ops;
  pub use crate::scheduler::*;
  pub use crate::subject;
  pub use crate::subject::*;
  pub use crate::subscriber::*;
  pub use crate::subscription;
  pub use crate::subscription::*;
  pub use crate::type_hint::TypeHint;
  pub use observer::Observer;

  #[cfg(not(target_arch = "wasm32"))]
  pub use std::time::{Duration, Instant};

  #[cfg(target_arch = "wasm32")]
  pub use web_time::{Duration, Instant};
}
