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
//! If you intentionally need feedback loops, insert an explicit async boundary
//! (e.g. `delay(Duration::from_millis(0))`).
//!
//! ```rust
//! use std::time::Duration;
//!
//! use rxrust::prelude::*;
//!
//! let subject = Local::subject();
//! let s_clone = subject.clone();
//!
//! let subscription = subject.clone().subscribe(move |_: ()| {
//!   // CORRECT: Insert an explicit async boundary before emitting.
//!   s_clone
//!     .clone()
//!     .delay(Duration::from_millis(0))
//!     .subscribe(|_| println!("Runs on a later tick"));
//! });
//!
//! subscription.unsubscribe();
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
