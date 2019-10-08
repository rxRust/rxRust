//! Reactive extensions library for Rust: a library for [Reactive Programming](http://reactivex.io/) using Observables, to make it easier to compose asynchronous or callback-based code. 
//! There are two core abstractions that are unique to RxRust:
//! * **[IntoShared](prelude::IntoShared):** By default, RxRust always provides a single thread version to get the best performance, but a thread-safe implementation also exists. The trait `IntoShared` will convert a local-thread struct to thread-safe version. So we can call `to_shared` method to ensure operators or subscription can shared between threads.
//! * **[Fork](prelude::IntoShared):** In Rxrust all operators consume the upstream except `Fork`, so operators always combine a single-chain and can only subscribe once. We use `Fork` to fork the stream.
#![feature(
  external_doc,
  fn_traits,
  step_trait,
  specialization,
  unboxed_closures,
  drain_filter,
  test,
  mem_take,
  decl_macro
)]
#[doc(include = "../README.md")]
#[macro_use]
extern crate lazy_static;

pub mod observable;
pub mod ops;
pub mod scheduler;
pub mod subject;
pub mod subscribable;
pub mod subscriber;
pub mod subscription;

pub mod prelude {
  pub use crate::observable;
  pub use crate::observable::Observable;
  pub use crate::ops;
  pub use crate::scheduler::*;
  pub use crate::subject;
  pub use crate::subject::Subject;
  pub use crate::subscribable;
  pub use crate::subscribable::*;
  pub use crate::subscriber;
  pub use crate::subscriber::Subscriber;
  pub use crate::subscription;
  pub use crate::subscription::*;
  pub use ops::Fork;
}
