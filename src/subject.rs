//! Subject implementations
//!
//! This module implements the Subject pattern for rxRust v1.0.
//!
//! # Architecture
//!
//! The Subject implementation in rxRust v1.0 is designed with the following key
//! goals:
//!
//! 1. **Pointer-Based Design**: The Subject is parameterized by a single smart
//!    pointer type `P`, which points to the subscribers container. This
//!    decouples the Subject from the `Context` trait and allows for greater
//!    flexibility.
//! 2. **Re-Entrancy Safety**: Emissions (`next`/`error`/`complete`) are not
//!    re-entrant to keep ordering predictable and prevent accidental feedback
//!    loops.
//!
//! # Core Concepts
//!
//! ## Pointer Abstraction
//!
//! Instead of binding the Subject to a `Context` trait directly, we use a
//! single `Subject<P>` struct where `P` is a smart pointer type (e.g.,
//! `Rc<RefCell<...>>` or `Arc<Mutex<...>>`). This allows the same `Subject`
//! struct to be used for both single-threaded (`Local`) and multi-threaded
//! (`Shared`) scenarios by simply changing the pointer type.
//!
//! ## Subscribers Container
//!
//! The `Subscribers` container uses `SmallVec` to optimize for the common case
//! of having few subscribers (<= 2), avoiding heap allocations for these
//! scenarios.
//!
//! ## Re-borrowing Support
//!
//! The Subject implementation supports broadcasting `&mut Item` via
//! re-borrowing, allowing for sequential modification of data in a subscription
//! chain.
//!
//! # Usage
//!
//! Subjects can be created using the `Local` or `Shared` context factories:
//!
//! ```rust
//! use rxrust::prelude::*;
//!
//! // Local Subject
//! let subject = Local::subject();
//! subject
//!   .clone()
//!   .subscribe(|v: i32| println!("Local: {}", v));
//! subject.clone().next(1);
//!
//! // Shared Subject
//! let shared_subject = Shared::subject();
//! shared_subject
//!   .clone()
//!   .subscribe(|v: i32| println!("Shared: {}", v));
//! shared_subject.clone().next(1);
//! ```
//!
//! ## Re-Entrancy Policy
//!
//! - **Emissions (`next`/`error`/`complete`) are not re-entrant**. Calling
//!   `next`/`error`/`complete` on the same Subject from within one of its own
//!   callbacks will **panic**.
//! - **Subscription mutations are allowed** (`subscribe`/`unsubscribe`) inside
//!   callbacks. They may be applied after the current emission finishes.
//!
//! If you intentionally need feedback loops (emitting values from within a
//! callback), insert an explicit async boundary using `delay`:
//!
//! ```rust,no_run
//! # #[cfg(not(target_arch = "wasm32"))]
//! # {
//! use std::convert::Infallible;
//!
//! use rxrust::prelude::*;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let mut subject = Shared::subject::<i32, Infallible>();
//! let mut emitter = subject.clone();
//!
//! // Apply delay(0) to the observable chain to create an async boundary.
//! // This schedules the callback on the next scheduler tick, after the
//! // original emission has completed and the Subject's internal borrow
//! // has been released.
//! subject
//!   .clone()
//!   .delay(Duration::from_millis(0))
//!   .subscribe(move |n| {
//!     println!("Received: {}", n);
//!     if n < 3 {
//!       // Safe: delay(0) ensures we're on a new scheduler tick
//!       emitter.next(n + 1);
//!     }
//!   });
//!
//! subject.next(0);
//! tokio::time::sleep(Duration::from_millis(100)).await;
//! # }
//! # }
//! ```

pub mod behavior_subject;
pub mod subject_core;
pub mod subject_subscription;
pub mod subscribers;

// Re-export the main types for convenience
pub use behavior_subject::*;
pub use subject_core::*;
pub use subject_subscription::*;
pub use subscribers::*;

// Re-export BoxedObserver types from observer module for backward compatibility
pub use crate::observer::{
  BoxedObserver, BoxedObserverMutRef, BoxedObserverMutRefSend, BoxedObserverSend,
};
