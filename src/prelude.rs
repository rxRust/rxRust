//! Prelude module for convenient imports
//!
//! This module re-exports commonly used types and traits for easy access.

// Context types and traits
// Factory trait
// Boxed types (Generic)
// Connectable trait (unified fork/connect/ref_count)
// Core traits
// Creation/Factories
// Scheduler-specific Observable aliases
#[cfg(feature = "scheduler")]
pub use crate::observable::{
  LocalBoxedObservable, LocalBoxedObservableClone, LocalBoxedObservableMutRef,
  LocalBoxedObservableMutRefClone, SharedBoxedObservable, SharedBoxedObservableClone,
  SharedBoxedObservableMutRef, SharedBoxedObservableMutRefClone,
};
// Observer trait
pub use crate::observer::Observer;
// Operators
pub use crate::ops::{into_future::*, into_stream::*};
// Test Scheduler
#[cfg(test)]
pub use crate::scheduler::test_scheduler::TestScheduler;
// Scheduler Core types
pub use crate::scheduler::{Duration, Instant, Schedulable, Scheduler};
// Default Schedulers
#[cfg(feature = "scheduler")]
pub use crate::scheduler::{LocalScheduler, SharedScheduler};
// Scheduler Task types
pub use crate::scheduler::{SleepProvider, Task, TaskHandle, TaskState};
// Subject
pub use crate::subject::*;
// Subscription
pub use crate::subscription::*;
pub use crate::{
  context::*,
  factory::ObservableFactory,
  observable::{
    BoxedCoreObservable, BoxedCoreObservableClone, BoxedCoreObservableMutRef,
    BoxedCoreObservableMutRefClone, BoxedCoreObservableMutRefSend,
    BoxedCoreObservableMutRefSendClone, BoxedCoreObservableSend, BoxedCoreObservableSendClone,
    Connectable, ConnectableObservable, CoreObservable, Defer, DynCoreObservable,
    DynCoreObservableClone, Empty, FromFn, FromFuture, FromIter, FromStream, Interval,
    IntoBoxedCoreObservable, Never, Observable, ObservableType, Of, ThrowErr, Timer,
  },
  ops::throttle::ThrottleEdge,
};
