//! Reactive extensions library for Rust: a library for
//! [Reactive Programming](http://reactivex.io/) using
//! [Observable](crate::observable::Observable), to make
//! it easier to compose asynchronous or callback-based code.

#[cfg(test)]
extern crate float_cmp;

#[cfg(test)]
#[macro_use]
extern crate bencher;

#[cfg(test)]
pub mod test_scheduler;

pub mod behavior_subject;
pub mod observable;
pub mod observer;
pub mod ops;
pub mod scheduler;
pub mod shared;
pub mod subject;
pub mod subscription;
pub mod type_hint;

pub mod prelude {
  pub use crate::behavior_subject;
  pub use crate::behavior_subject::*;
  pub use crate::observable;
  pub use crate::observable::*;
  pub use crate::observer;
  pub use crate::ops;
  pub use crate::scheduler::*;
  pub use crate::shared;
  pub use crate::subject;
  pub use crate::subject::*;
  pub use crate::subscription;
  pub use crate::subscription::*;
  pub use crate::type_hint::TypeHint;
  pub use observer::Observer;
  pub use shared::*;
}
