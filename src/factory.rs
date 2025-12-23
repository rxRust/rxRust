//! Observable factory pattern
//!
//! This module introduces the `ObservableFactory` trait, which provides a
//! convenient way to create observable sequences (e.g., via `of`, `from_iter`,
//! `timer`).
//!
//! The `ObservableFactory` trait is designed to be automatically implemented
//! for any type that satisfies the necessary `Context` trait bounds. This means
//! users typically do not implement `ObservableFactory` directly, but rather
//! implement `Context` for their custom types, which then automatically gain
//! factory capabilities.
//!
//! This design eliminates boilerplate, reduces API duplication, and allows for
//! flexible scheduler injection by relying on the `Context` to provide the
//! execution environment.
//!
//! ## Usage
//!
//! `ObservableFactory` methods are available on any type that implements
//! `Context` and whose associated `Scheduler` type implements `Default`. The
//! most common way to use this is through the default `Local` and `Shared`
//! marker types provided in `rxrust::prelude`, or by defining your own custom
//! `Context` types (see `rxrust::context` documentation for details on custom
//! `Context` implementations).
//!
//! ### Examples
//!
//! ```rust,no_run
//! use rxrust::prelude::*;
//!
//! // Create a local observable that emits 42 and completes.
//! // `Local` here refers to `rxrust::prelude::Local`,
//! // which implements `Context` with `LocalScheduler`.
//! Local::of(42).subscribe(|v| println!("Received local: {}", v));
//!
//! // Create a shared observable that emits "hello" and completes.
//! // `Shared` here refers to `rxrust::prelude::Shared`,
//! // which implements `Context` with `SharedScheduler`.
//! Shared::of("hello").subscribe(|v| println!("Received shared: {}", v));
//!
//! // Create a timer that fires after 500ms
//! Local::timer(Duration::from_millis(500)).subscribe(|_| println!("Timer fired!"));
//! ```
//!
//! ## Trivial Observables
//!
//! This factory provides several methods for creating "trivial" observables
//! that emit limited or no values. These are particularly useful for testing,
//! edge cases, and specific reactive patterns:
//!
//! ### Available Trivial Observables
//!
//! | Method | Description | Completion | Values Emitted | Error Emitted |
//! |--------|-------------|------------|----------------|--------------|
//! | `empty()` | Completes immediately without emitting any values | ✅ Yes | None | None |
//! | `never()` | Never emits any values and never completes | ❌ No | None | None |
//! | `throw_err()` | Immediately emits an error without any values | ❌ No | None | Yes |
//!
//! ### Use Cases for Trivial Observables
//!
//! - **Testing**: Create predictable streams for unit tests
//! - **Fallback scenarios**: Provide default behavior when no data is available
//! - **Error handling**: Simulate failure conditions
//! - **Infinite streams**: Create streams that require manual termination
//! - **Base cases**: Handle edge cases in conditional observable chains
//!
//! #### `empty()` - Immediate Completion
//! ```rust
//! use rxrust::prelude::*;
//!
//! // Useful for representing empty collections or completion signals
//! Local::empty()
//!   .on_complete(|| println!("Stream completed immediately"))
//!   .subscribe(|v| println!("This won't be called"));
//! ```
//!
//! #### `never()` - Infinite Stream
//! ```rust
//! use rxrust::prelude::*;
//!
//! // Useful for testing timeout or cancellation behavior
//! let subscription = Local::never().subscribe(|v| println!("This won't be called"));
//!
//! // Must manually unsubscribe to avoid memory leaks
//! drop(subscription);
//! ```
//!
//! #### `throw_err()` - Immediate Error
//! ```rust
//! use rxrust::prelude::*;
//!
//! // Useful for testing error handling or representing failures
//! Local::throw_err("Network error".to_string())
//!   .on_error(|e| println!("Error occurred: {}", e))
//!   .subscribe(|v| println!("This won't be called"));
//! ```
//!
//! For detailed information about each observable implementation, see the
//! [`trivial`](crate::observable::trivial) module.

// Standard library imports
// Internal module imports
use crate::{
  context::Context,
  observable::{defer::Defer, *},
  observer::Emitter,
  scheduler::{Duration, Instant},
  subject::{BehaviorSubject, Subject, SubjectPtr, SubjectPtrMutRef},
  subscription::Subscription,
};

/// `ObservableFactory` trait for creating observable sequences.
///
/// This trait provides factory methods (like `of`, `from_iter`, `timer`) to
/// construct new observable streams. It is designed to work seamlessly with the
/// `Context` trait to allow for flexible scheduler injection.
///
/// A blanket implementation ensures that any type `C` that implements `Context`
/// and whose associated `Scheduler` type implements `Default`, automatically
/// gains `ObservableFactory` capabilities.
pub trait ObservableFactory: Context<Inner = ()> {
  /// Creates an Observable from a closure that defines the subscription logic.
  ///
  /// This method allows creating an Observable by providing a function that
  /// receives an `Emitter` and returns a `Subscription`.
  ///
  /// The `Emitter` interface uses mutable references (`&mut self`) for all
  /// methods, avoiding heap allocation (boxing) of the observer.
  ///
  /// # Arguments
  /// * `f` - The subscription function. It receives an `Emitter` and returns
  ///   teardown logic.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// Local::create(|emitter| {
  ///   emitter.next(1);
  ///   emitter.complete();
  ///   // Return teardown logic (e.g., a subscription or closure)
  ///   ()
  /// })
  /// .subscribe(|v: i32| println!("{}", v));
  /// ```
  fn create<Item, Err, F, U>(f: F) -> Self::With<Create<F, Item, Err>>
  where
    F: FnOnce(&mut dyn Emitter<Item, Err>) -> U,
    U: Subscription,
  {
    Self::lift(Create::new(f))
  }

  /// Creates an observable that emits a single value and then completes.
  ///
  /// The returned observable will use the default scheduler associated with
  /// `Self`'s `Context`.
  ///
  /// # Arguments
  /// * `v` - The value to be emitted by the observable.
  ///
  /// # Returns
  /// A new observable (`Self::With<Of<T>>`) that emits `v` and completes.
  fn of<V>(v: V) -> Self::With<Of<V>> {
    Self::lift(Of(v)) // Uses the `Context::lift` method with the default scheduler.
  }

  /// Creates an observable that emits no items and completes immediately.
  ///
  /// This is useful for representing empty collections, completion signals,
  /// or as a base case in conditional observable chains.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use std::{cell::RefCell, rc::Rc};
  ///
  /// use rxrust::prelude::*;
  ///
  /// let completed = Rc::new(RefCell::new(false));
  /// let completed_clone = completed.clone();
  ///
  /// Local::empty()
  ///   .on_complete(move || *completed_clone.borrow_mut() = true)
  ///   .subscribe(|_| println!("This won't be called"));
  ///
  /// assert_eq!(*completed.borrow(), true);
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::never`] - Creates an observable that never completes
  /// * [`Self::throw_err`] - Creates an observable that immediately errors
  /// * [`Empty`] - The underlying observable implementation
  fn empty() -> Self::With<Empty> { Self::lift(Empty) }

  /// Creates an observable that emits no items and never completes.
  ///
  /// This represents an infinite stream that never terminates naturally.
  /// It's commonly used for testing timeout behavior, creating infinite
  /// streams, or implementing retry mechanisms.
  ///
  /// # Warning
  ///
  /// Since this observable never completes, you should typically use operators
  /// like `take()`, `timeout()`, or manually unsubscribe to avoid memory leaks.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// // In practice, you'd typically combine this with operators like take()
  /// let subscription = Local::never()
  ///   .take(3) // Never emits, so take won't get any values
  ///   .subscribe(|_| println!("Got"));
  ///
  /// // You must manually unsubscribe since it never completes
  /// drop(subscription);
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::empty`] - Creates an observable that completes immediately
  /// * [`Self::throw_err`] - Creates an observable that immediately errors
  /// * [`Never`] - The underlying observable implementation
  fn never() -> Self::With<Never> { Self::lift(Never) }

  /// Creates an observable that emits no items and terminates with an error
  /// immediately.
  ///
  /// This is useful for testing error handling, creating error-based fallbacks,
  /// or representing failure conditions as observables.
  ///
  /// # Arguments
  ///
  /// * `error` - The error to emit when the observable is subscribed to
  ///
  /// # Examples
  ///
  /// ```rust
  /// use std::{cell::RefCell, rc::Rc};
  ///
  /// use rxrust::prelude::*;
  ///
  /// let error_received = Rc::new(RefCell::new(None));
  /// let completed = Rc::new(RefCell::new(false));
  ///
  /// let error_received_clone = error_received.clone();
  /// let completed_clone = completed.clone();
  ///
  /// Local::throw_err("Connection failed".to_string())
  ///   .on_complete(move || *completed_clone.borrow_mut() = true)
  ///   .on_error(move |e| *error_received_clone.borrow_mut() = Some(e))
  ///   .subscribe(|_| println!("This won't be called"));
  ///
  /// assert_eq!(*completed.borrow(), false);
  /// assert_eq!(*error_received.borrow(), Some("Connection failed".to_string()));
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::empty`] - Creates an observable that completes immediately
  /// * [`Self::never`] - Creates an observable that never completes
  /// * [`ThrowErr`] - The underlying observable implementation
  fn throw_err<E>(error: E) -> Self::With<ThrowErr<E>> { Self::lift(ThrowErr { error }) }

  /// Creates a new Subject.
  ///
  /// This method creates a `Subject` that can act as both an `Observer` and an
  /// `Observable`. It allows you to multicast values to multiple subscribers.
  /// The `Subject` starts without any initial value and emits values only
  /// after they are subscribed to.
  ///
  /// # Type Parameters
  ///
  /// * `Item` - The type of items emitted by the `Subject`.
  /// * `Err` - The type of errors emitted by the `Subject`.
  ///
  /// # Returns
  /// A `Subject` wrapped in the context's associated type.
  fn subject<'a, Item, Err>() -> Self::With<Subject<SubjectPtr<'a, Self, Item, Err>>> {
    Self::lift(Subject::default())
  }

  /// Creates a new Subject with mutable reference observer.
  ///
  /// This method creates a `Subject` that uses mutable reference observers. It
  /// is useful when you need to share mutable state between observers.
  ///
  /// # Type Parameters
  ///
  /// * `Item` - The type of items emitted by the `Subject`.
  /// * `Err` - The type of errors emitted by the `Subject`.
  ///
  /// # Returns
  /// A `Subject` with mutable reference observers wrapped in the context's
  /// associated type.
  fn subject_mut_ref<'a, Item, Err>() -> Self::With<Subject<SubjectPtrMutRef<'a, Self, Item, Err>>>
  {
    Self::lift(Subject::default())
  }

  /// Creates a new BehaviorSubject.
  ///
  /// This method creates a `BehaviorSubject` that emits the most recent item it
  /// has observed and all subsequent items to its subscribers. It requires an
  /// initial value to be provided.
  ///
  /// # Type Parameters
  ///
  /// * `Item` - The type of items emitted by the `BehaviorSubject`. Must
  ///   implement `Clone`.
  /// * `Err` - The type of errors emitted by the `BehaviorSubject`.
  ///
  /// # Arguments
  ///
  /// * `initial` - The initial value to be emitted by the `BehaviorSubject`.
  ///
  /// # Returns
  /// A `BehaviorSubject` wrapped in the context's associated type.
  fn behavior_subject<'a, Item: Clone, Err>(
    initial: Item,
  ) -> Self::With<BehaviorSubject<Item, SubjectPtr<'a, Self, Item, Err>>> {
    Self::lift(BehaviorSubject::new(initial))
  }

  /// Creates a new BehaviorSubject with mutable reference observer.
  ///
  /// This method creates a `BehaviorSubject` that uses mutable reference
  /// observers. It emits the most recent item it has observed and all
  /// subsequent items to its subscribers. It requires an initial value to be
  /// provided.
  ///
  /// # Type Parameters
  ///
  /// * `Item` - The type of items emitted by the `BehaviorSubject`. Must
  ///   implement `Clone`.
  /// * `Err` - The type of errors emitted by the `BehaviorSubject`.
  ///
  /// # Arguments
  ///
  /// * `initial` - The initial value to be emitted by the `BehaviorSubject`.
  ///
  /// # Returns
  /// A `BehaviorSubject` with mutable reference observers wrapped in the
  /// context's associated type.
  fn behavior_subject_mut_ref<'a, Item: Clone + 'a, Err>(
    initial: Item,
  ) -> Self::With<BehaviorSubject<Item, SubjectPtrMutRef<'a, Self, Item, Err>>> {
    Self::lift(BehaviorSubject::new(initial))
  }

  /// Creates an observable from an iterator that emits each item synchronously
  /// when subscribed.
  ///
  /// This observable emits all items from the iterator in order when
  /// subscribed, then completes. It's useful for converting collections and
  /// other iterables into observable streams.
  ///
  /// # Arguments
  ///
  /// * `iter` - An iterator to get all the values from.
  ///
  /// # Type Parameters
  ///
  /// * `I` - The iterator type that will be converted to an observable
  ///
  /// # Examples
  ///
  /// ```rust
  /// use std::convert::Infallible;
  ///
  /// use rxrust::prelude::*;
  ///
  /// // Create an observable from a range
  /// Local::from_iter(0..5).subscribe(|v| println!("Value: {}", v));
  /// // Output: Value: 0, Value: 1, Value: 2, Value: 3, Value: 4
  ///
  /// // Create an observable from a vector
  /// Local::from_iter(vec![1, 2, 3]).subscribe(|v| println!("Value: {}", v));
  /// // Output: Value: 1, Value: 2, Value: 3
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::of`] - Creates an observable that emits a single value
  /// * [`Self::empty`] - Creates an observable that completes without emitting
  ///   values
  /// * [`FromIter`] - The underlying observable implementation
  fn from_iter<I: IntoIterator>(iter: I) -> Self::With<FromIter<I>> { Self::lift(from_iter(iter)) }

  /// Creates an observable that emits a single value generated by a function at
  /// subscription time.
  ///
  /// This operator is useful when you need to generate a value at subscription
  /// time, such as when the value depends on the current state of the system.
  ///
  /// # Arguments
  ///
  /// * `f` - A function that generates the value to emit when the observable is
  ///   subscribed to
  ///
  /// # Type Parameters
  ///
  /// * `F` - The function type that generates the value
  ///
  /// # Examples
  ///
  /// ```rust
  /// use std::convert::Infallible;
  ///
  /// use rxrust::prelude::*;
  ///
  /// // Generate a value at subscription time
  /// Local::from_fn(|| 42).subscribe(|v| println!("Got: {}", v));
  /// // Output: Got: 42
  ///
  /// // Generate a value based on current state
  /// let counter = std::rc::Rc::new(std::cell::RefCell::new(0));
  /// let counter_clone = counter.clone();
  ///
  /// Local::from_fn(move || {
  ///   *counter_clone.borrow_mut() += 1;
  ///   *counter_clone.borrow()
  /// })
  /// .subscribe(|v| println!("Counter value: {}", v));
  /// // Output: Counter value: 1
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::of`] - Creates an observable that emits a predetermined single
  ///   value
  /// * [`Self::defer`] - Creates an observable that generates a new observable
  ///   at subscription time
  /// * [`FromFn`] - The underlying observable implementation
  fn from_fn<F>(f: F) -> Self::With<FromFn<F>> { Self::lift(FromFn(f)) }

  /// Creates an observable that calls a factory function to generate a new
  /// observable for each subscriber.
  ///
  /// The factory function is invoked lazily at subscription time, allowing you
  /// to:
  /// - Create observables based on current state
  /// - Generate fresh observable sequences for each subscriber
  /// - Defer expensive observable construction until needed
  ///
  /// # Key Distinction
  ///
  /// Unlike [`Self::from_fn`], which creates an observable that emits a
  /// **value**, `defer()` creates an observable that emits an entire
  /// **observable sequence**. This allows for dynamic observable construction
  /// based on runtime conditions.
  ///
  /// # Arguments
  ///
  /// * `f` - A factory function that returns an observable when called at
  ///   subscription time
  ///
  /// # Examples
  ///
  /// ```rust
  /// use std::{cell::RefCell, rc::Rc};
  ///
  /// use rxrust::prelude::*;
  ///
  /// // Create different observables based on state at subscription time
  /// let counter = Rc::new(RefCell::new(0));
  /// let counter_clone = counter.clone();
  ///
  /// let observable = Local::defer(move || {
  ///   let count = *counter_clone.borrow();
  ///   if count % 2 == 0 { Local::of("even") } else { Local::of("odd") }
  /// });
  ///
  /// observable
  ///   .clone()
  ///   .subscribe(|v| println!("First: {}", v)); // Output: First: even
  ///
  /// *counter.borrow_mut() = 1;
  /// observable.subscribe(|v| println!("Second: {}", v)); // Output: Second: odd
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::from_fn`] - Generates a **value** at subscription time (emits
  ///   one item)
  /// * [`Self::of`] - Emits a predetermined value (no lazy evaluation)
  /// * [`Defer`] - The underlying observable implementation
  fn defer<F, O>(f: F) -> Self::With<Defer<F, O>>
  where
    F: FnOnce() -> Self::With<O>,
    O: ObservableType,
  {
    Self::lift(Defer::new(f))
  }

  /// Creates an observable that emits a single value after a specified delay.
  ///
  /// Emits `()` after the specified delay, then completes. Uses the default
  /// scheduler from the current `Context`.
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use rxrust::prelude::*;
  ///
  /// Local::timer(Duration::from_millis(100)).subscribe(|_| println!("Timer fired after 100ms"));
  ///
  /// // For emitting a specific value after delay:
  /// Local::timer(Duration::from_millis(100))
  ///   .map(|_| 42)
  ///   .subscribe(|v| println!("Got: {}", v));
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::timer_with`] - Same functionality with custom scheduler
  /// * `delay()` - Delays emissions from an existing observable
  fn timer(delay: Duration) -> Self::With<Timer<Self::Scheduler>> {
    Self::lift(Timer { delay, scheduler: Self::Scheduler::default() })
  }

  /// Creates an observable that emits a single value after a specified delay,
  /// using a custom scheduler.
  ///
  /// Same as [`Self::timer`], but uses a custom scheduler instead of the
  /// default one. Useful for using `SharedScheduler` in `Local` context or
  /// for specific scheduling behavior.
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use rxrust::prelude::*;
  ///
  /// // Use SharedScheduler in Local context
  /// Local::timer_with(Duration::from_millis(50), SharedScheduler)
  ///   .subscribe(|_| println!("Timer fired with SharedScheduler"));
  /// ```
  fn timer_with<S>(delay: Duration, scheduler: S) -> Self::With<Timer<S>> {
    Self::lift(Timer { delay, scheduler })
  }

  /// Creates an observable that emits `()` at a specific time `at`.
  ///
  /// If `at` is in the past, emits immediately.
  fn timer_at(at: Instant) -> Self::With<Timer<Self::Scheduler>> {
    let now = Instant::now();
    let delay = if at > now { at - now } else { Duration::default() };
    Self::lift(Timer { delay, scheduler: Self::Scheduler::default() })
  }

  /// Creates an observable that emits `()` at a specific time `at`, using a
  /// custom scheduler.
  fn timer_at_with<S>(at: Instant, scheduler: S) -> Self::With<Timer<S>> {
    let now = Instant::now();
    let delay = if at > now { at - now } else { Duration::default() };
    Self::lift(Timer { delay, scheduler })
  }

  /// Creates an observable that emits sequential numbers at regular intervals.
  ///
  /// Emits incrementing `usize` values (0, 1, 2, ...) at the specified
  /// interval. The observable continues indefinitely until unsubscribed.
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use rxrust::prelude::*;
  ///
  /// Local::interval(Duration::from_millis(100))
  ///   .take(5) // Take only 5 emissions
  ///   .subscribe(|n| println!("Tick {}", n));
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::interval_with`] - Same functionality with custom scheduler
  /// * [`Self::timer`] - Emits a single value after a delay
  fn interval(period: Duration) -> Self::With<Interval<Self::Scheduler>> {
    Self::lift(Interval { period, scheduler: Self::Scheduler::default() })
  }

  /// Creates an observable that emits sequential numbers at regular intervals,
  /// using a custom scheduler.
  ///
  /// Same as [`Self::interval`], but uses a custom scheduler instead of the
  /// default one.
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use rxrust::prelude::*;
  ///
  /// // Use SharedScheduler in Local context
  /// Local::interval_with(Duration::from_millis(50), SharedScheduler)
  ///   .take(3)
  ///   .subscribe(|n| println!("Tick {}", n));
  /// ```
  fn interval_with<S>(period: Duration, scheduler: S) -> Self::With<Interval<S>> {
    Self::lift(Interval { period, scheduler })
  }

  /// Creates an observable from a `Future` that emits a single value when the
  /// future completes.
  ///
  /// The returned observable will await the future's completion and emit its
  /// result, then complete. It uses the default scheduler associated with
  /// `Self`'s `Context`.
  ///
  /// # Type Parameters
  ///
  /// * `F` - The future type
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use std::future;
  ///
  /// use rxrust::prelude::*;
  ///
  /// Local::from_future(future::ready(42)).subscribe(|v| println!("Got: {}", v));
  ///
  /// // Chain with operators
  /// Local::from_future(future::ready(10))
  ///   .map(|v| v * 2)
  ///   .subscribe(|v| println!("Doubled: {}", v));
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::from_future_with`] - Same functionality with custom scheduler
  /// * [`Self::from_stream`] - Converts a `Stream` to an observable
  /// * [`Self::of`] - Creates an observable from a single value
  fn from_future<F: std::future::Future>(
    future: F,
  ) -> Self::With<from_future::FromFuture<F, Self::Scheduler>> {
    Self::lift(from_future::FromFuture { future, scheduler: Self::Scheduler::default() })
  }

  /// Creates an observable from a `Future` using a custom scheduler.
  ///
  /// Same as [`Self::from_future`], but uses a custom scheduler instead of the
  /// default one. Useful for using `SharedScheduler` in `Local` context or
  /// for specific scheduling behavior.
  ///
  /// # Type Parameters
  ///
  /// * `F` - The future type
  /// * `S` - The scheduler type
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use std::future;
  ///
  /// use rxrust::prelude::*;
  ///
  /// // Use SharedScheduler in Local context
  /// Local::from_future_with(future::ready("hello"), SharedScheduler)
  ///   .subscribe(|v| println!("Got: {}", v));
  ///
  /// // Note: SharedScheduler can be used in Local context,
  /// // but LocalScheduler cannot be used in Shared context
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::from_future`] - Same functionality with default scheduler
  fn from_future_with<F, S>(future: F, scheduler: S) -> Self::With<from_future::FromFuture<F, S>> {
    Self::lift(from_future::FromFuture { future, scheduler })
  }

  /// Creates an observable from a `Future` that returns a `Result` type.
  ///
  /// Unlike [`Self::from_future`], this method handles `Result` types
  /// specially:
  /// - `Ok(value)` is emitted via `next()` followed by `complete()`
  /// - `Err(error)` is emitted via `error()`
  ///
  /// This is useful when working with async operations that can fail, allowing
  /// you to handle errors through the observable's error channel.
  ///
  /// # Type Parameters
  ///
  /// * `F` - The future type that outputs `Result<Item, Err>`
  /// * `Item` - The success type
  /// * `Err` - The error type
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use std::future;
  ///
  /// use rxrust::prelude::*;
  ///
  /// // Success case - emits value and completes
  /// Local::from_future_result(future::ready(Ok::<_, String>(42)))
  ///   .on_error(|_e| {})
  ///   .subscribe(|v| println!("Got: {}", v));
  ///
  /// // Error case - emits error
  /// Local::from_future_result(future::ready(Err::<i32, _>("failed")))
  ///   .on_error(|e| println!("Error: {}", e))
  ///   .subscribe(|v| println!("Got: {}", v));
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::from_future_result_with`] - Same functionality with custom
  ///   scheduler
  /// * [`Self::from_future`] - For futures that don't return Result types
  fn from_future_result<F, Item, Err>(
    future: F,
  ) -> Self::With<from_future::FromFutureResult<F, Self::Scheduler>>
  where
    F: std::future::Future<Output = Result<Item, Err>>,
  {
    Self::lift(from_future::FromFutureResult { future, scheduler: Self::Scheduler::default() })
  }

  /// Creates an observable from a `Future` that returns a `Result` type, using
  /// a custom scheduler.
  ///
  /// Same as [`Self::from_future_result`], but uses a custom scheduler instead
  /// of the default one.
  ///
  /// # Type Parameters
  ///
  /// * `F` - The future type that outputs `Result<Item, Err>`
  /// * `S` - The scheduler type
  /// * `Item` - The success type
  /// * `Err` - The error type
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use std::future;
  ///
  /// use rxrust::prelude::*;
  ///
  /// // Use SharedScheduler in Local context
  /// Local::from_future_result_with(future::ready(Ok::<_, String>(42)), SharedScheduler)
  ///   .on_error(|_e| {})
  ///   .subscribe(|v| println!("Got: {}", v));
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::from_future_result`] - Same functionality with default scheduler
  fn from_future_result_with<F, S, Item, Err>(
    future: F, scheduler: S,
  ) -> Self::With<from_future::FromFutureResult<F, S>>
  where
    F: std::future::Future<Output = Result<Item, Err>>,
  {
    Self::lift(from_future::FromFutureResult { future, scheduler })
  }

  /// Creates an observable from a `futures_core::stream::Stream` that emits
  /// each item from the stream.
  ///
  /// This method converts a `futures_core::stream::Stream` into an Observable
  /// sequence. The stream is polled and each item is emitted to the observer.
  /// When the stream is exhausted (returns `Ready(None)`), the observable
  /// completes.
  ///
  /// # The futures_core::stream::Stream Pattern
  ///
  /// The `futures_core::stream::Stream` trait is the standard async iterator in
  /// Rust:
  /// - `Ready(Some(value))` → Emit value, continue polling
  /// - `Ready(None)` → Stream exhausted, complete
  /// - `Pending` → No value yet, yield control
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use futures::stream;
  /// use rxrust::prelude::*;
  ///
  /// // Create an observable from a futures_core::Stream generated from an iterator
  /// let stream = stream::iter(vec![1, 2, 3]);
  /// Local::from_stream(stream).subscribe(|v| println!("Got: {}", v));
  /// // Output: Got: 1, Got: 2, Got: 3
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::from_stream_with`] - Same functionality with custom scheduler
  /// * [`Self::from_future`] - Converts a single `Future` to an observable
  fn from_stream<St>(stream: St) -> Self::With<from_stream::FromStream<St, Self::Scheduler>>
  where
    St: futures_core::stream::Stream,
  {
    Self::lift(from_stream::FromStream { stream, scheduler: Self::Scheduler::default() })
  }

  /// Creates an observable from a `futures_core::stream::Stream` using a custom
  /// scheduler.
  ///
  /// Same as [`Self::from_stream`], but uses a custom scheduler instead of the
  /// default one. Useful for using `SharedScheduler` in `Local` context or
  /// for specific scheduling behavior.
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use futures::stream;
  /// use rxrust::prelude::*;
  ///
  /// // Use SharedScheduler in Local context
  /// let stream = stream::iter(vec!["Hello", "World"]);
  /// Local::from_stream_with(stream, SharedScheduler).subscribe(|v| println!("Got: {}", v));
  /// ```
  fn from_stream_with<St, S>(stream: St, scheduler: S) -> Self::With<from_stream::FromStream<St, S>>
  where
    St: futures_core::stream::Stream,
  {
    Self::lift(from_stream::FromStream { stream, scheduler })
  }

  /// Creates an observable from a `Stream<Item = Result<Item, Err>>` that
  /// handles errors.
  ///
  /// Unlike [`Self::from_stream`], this method handles `Result` types
  /// specially:
  /// - `Ok(value)` is emitted via `next()`
  /// - `Err(error)` is emitted via `error()` and terminates the stream
  ///
  /// This is useful when working with async streams that can fail, allowing
  /// you to handle errors through the observable's error channel.
  ///
  /// # Type Parameters
  ///
  /// * `St` - The stream type that outputs `Result<Item, Err>`
  /// * `Item` - The success type
  /// * `Err` - The error type
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use std::convert::Infallible;
  ///
  /// use futures::stream;
  /// use rxrust::prelude::*;
  ///
  /// // Success case - emits values and completes
  /// let stream = stream::iter(vec![Ok::<i32, Infallible>(1), Ok(2), Ok(3)]);
  /// Local::from_stream_result(stream)
  ///   .on_error(|_e| {})
  ///   .subscribe(|v| println!("Got: {}", v));
  ///
  /// // Error case - emits values until error, then terminates
  /// let stream = stream::iter(vec![Ok(1), Err("error"), Ok(3)]);
  /// Local::from_stream_result(stream)
  ///   .on_error(|e| println!("Error: {}", e))
  ///   .subscribe(|v| println!("Got: {}", v));
  /// // Output: Got: 1, Error: error
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::from_stream_result_with`] - Same functionality with custom
  ///   scheduler
  /// * [`Self::from_stream`] - For streams that don't emit Result types
  fn from_stream_result<St, Item, Err>(
    stream: St,
  ) -> Self::With<from_stream::FromStreamResult<St, Self::Scheduler>>
  where
    St: futures_core::stream::Stream<Item = Result<Item, Err>>,
  {
    Self::lift(from_stream::FromStreamResult { stream, scheduler: Self::Scheduler::default() })
  }

  /// Creates an observable from a `Stream<Item = Result<Item, Err>>` using a
  /// custom scheduler.
  ///
  /// Same as [`Self::from_stream_result`], but uses a custom scheduler instead
  /// of the default one.
  ///
  /// # Type Parameters
  ///
  /// * `St` - The stream type that outputs `Result<Item, Err>`
  /// * `S` - The scheduler type
  /// * `Item` - The success type
  /// * `Err` - The error type
  ///
  /// # Examples
  ///
  /// ```rust,no_run
  /// use std::convert::Infallible;
  ///
  /// use futures::stream;
  /// use rxrust::prelude::*;
  ///
  /// // Use SharedScheduler in Local context
  /// let stream = stream::iter(vec![Ok::<i32, Infallible>(1), Ok(2), Ok(3)]);
  /// Local::from_stream_result_with(stream, SharedScheduler)
  ///   .on_error(|_e| {})
  ///   .subscribe(|v| println!("Got: {}", v));
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::from_stream_result`] - Same functionality with default scheduler
  fn from_stream_result_with<St, S, Item, Err>(
    stream: St, scheduler: S,
  ) -> Self::With<from_stream::FromStreamResult<St, S>>
  where
    St: futures_core::stream::Stream<Item = Result<Item, Err>>,
  {
    Self::lift(from_stream::FromStreamResult { stream, scheduler })
  }

  // ==================== Combination Factory Methods ====================

  /// Merge multiple observables concurrently, subscribing to all at once.
  ///
  /// Subscribes to all observables simultaneously and emits values from any
  /// source as they arrive. The output order depends on when each source
  /// emits, not on the order of observables in the input.
  ///
  /// # Difference from `concat_observables`
  ///
  /// | Aspect | `merge_observables` | `concat_observables` |
  /// |--------|---------------------|----------------------|
  /// | Subscription | All at once (concurrent) | One at a time (sequential) |
  /// | Output order | Interleaved by emission time | Preserves source order |
  /// | Use case | Parallel processing | Sequential processing |
  ///
  /// # Marble Diagram
  ///
  /// ```text
  /// obs1: --1--2--|
  /// obs2: -3--4--|
  /// obs3: ---5-6--|
  ///
  /// merge_observables([obs1, obs2, obs3]):
  ///       -31-524-6--|
  ///       (interleaved based on timing)
  /// ```
  ///
  /// # Arguments
  ///
  /// * `observables` - An iterable of observables to merge
  ///
  /// # Completion
  ///
  /// Completes only when ALL source observables have completed.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let obs1 = Local::from_iter([1, 2]);
  /// let obs2 = Local::from_iter([3, 4]);
  /// let obs3 = Local::from_iter([5, 6]);
  ///
  /// // All subscribed at once, output order depends on emission timing
  /// Local::merge_observables([obs1, obs2, obs3]).subscribe(|v| println!("Got: {}", v));
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::concat_observables`] - Sequential subscription (one at a time)
  /// * [`Observable::merge`] - Instance method for merging two observables
  fn merge_observables<O, I>(
    observables: I,
  ) -> Self::With<crate::ops::merge_all::MergeAll<FromIter<I::IntoIter>>>
  where
    O: ObservableType,
    I: IntoIterator<Item = Self::With<O>>,
  {
    let observables = observables.into_iter();
    Self::lift(crate::ops::merge_all::MergeAll {
      source: from_iter(observables),
      concurrent: usize::MAX,
    })
  }

  /// Concatenate multiple observables sequentially, subscribing one at a time.
  ///
  /// Subscribes to observables one by one: waits for the first to complete
  /// before subscribing to the second, and so on. The output order is
  /// guaranteed to match the order of observables in the input.
  ///
  /// # Difference from `merge_observables`
  ///
  /// | Aspect | `concat_observables` | `merge_observables` |
  /// |--------|----------------------|---------------------|
  /// | Subscription | One at a time (sequential) | All at once (concurrent) |
  /// | Output order | Preserves source order | Interleaved by emission time |
  /// | Use case | Sequential processing | Parallel processing |
  ///
  /// # Marble Diagram
  ///
  /// ```text
  /// obs1: --1--2--|
  /// obs2: -3--4--|
  /// obs3: ---5-6--|
  ///
  /// concat_observables([obs1, obs2, obs3]):
  ///       --1--2--3--4--5-6--|
  ///       (sequential: obs1 completes, then obs2, then obs3)
  /// ```
  ///
  /// # Arguments
  ///
  /// * `observables` - An iterable of observables to concatenate
  ///
  /// # Completion
  ///
  /// Completes when the last observable in the sequence completes.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rxrust::prelude::*;
  ///
  /// let obs1 = Local::from_iter([1, 2]);
  /// let obs2 = Local::from_iter([3, 4]);
  /// let obs3 = Local::from_iter([5, 6]);
  ///
  /// // Sequential: obs1 first, then obs2, then obs3
  /// Local::concat_observables([obs1, obs2, obs3]).subscribe(|v| println!("{}", v));
  /// // Output guaranteed: 1, 2, 3, 4, 5, 6
  /// ```
  ///
  /// # See Also
  ///
  /// * [`Self::merge_observables`] - Concurrent subscription (all at once)
  /// * [`Observable::concat_all`] - Flatten a higher-order observable
  ///   sequentially
  fn concat_observables<O, I>(
    observables: I,
  ) -> Self::With<crate::ops::merge_all::MergeAll<FromIter<I::IntoIter>>>
  where
    O: ObservableType,
    I: IntoIterator<Item = Self::With<O>>,
  {
    let observables = observables.into_iter();
    Self::lift(crate::ops::merge_all::MergeAll {
      source: from_iter(observables),
      concurrent: 1, // Sequential execution
    })
  }
}

// Blanket implementation: Any `Context<Inner = ()>`, reuires `Inner = ()` to
//  avoid multiple blanket implementation
impl<C: Context<Inner = ()>> ObservableFactory for C {}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    context::{CellRc, MutRc},
    prelude::*,
    scheduler::{Task, TaskHandle},
  };

  // A minimal custom scheduler for testing the shadowing mechanism
  #[derive(Clone, Copy, Default)]
  struct CustomTestScheduler;

  impl<S> crate::scheduler::Schedulable<CustomTestScheduler> for Task<S> {
    type Future = std::future::Ready<()>;
    fn into_future(self, _scheduler: &CustomTestScheduler) -> Self::Future {
      std::future::ready(())
    }
  }

  impl<S> crate::scheduler::Scheduler<Task<S>> for CustomTestScheduler {
    fn schedule(&self, mut task: Task<S>, _delay: Option<Duration>) -> TaskHandle {
      // In a test, we can just run it immediately for simplicity
      task.step();
      TaskHandle::finished()
    }
  }

  // A custom context struct to demonstrate using a custom scheduler.
  // Since `Local` is now bound to `LocalScheduler`, we need our own struct
  // to carry a `CustomTestScheduler`.
  #[derive(Clone)]
  struct CustomContext<T> {
    inner: T,
    scheduler: CustomTestScheduler,
  }

  impl<T> Context for CustomContext<T> {
    type Inner = T;
    type Scheduler = CustomTestScheduler;
    type RcMut<U> = MutRc<U>; // Use MutRc since it's single-threaded like Local
    type RcCell<U: Copy + Eq> = CellRc<U>; // Use CellRc for Copy types
    type With<U> = CustomContext<U>;
    type BoxedObserver<'a, Item, Err> = BoxedObserver<'a, Item, Err>;
    type BoxedObserverMutRef<'a, Item: 'a, Err> = BoxedObserverMutRef<'a, Item, Err>;
    type BoxedSubscription = BoxedSubscription;
    type BoxedCoreObservable<'a, Item, Err> =
      crate::ops::box_it::BoxedCoreObservable<'a, Item, Err, CustomTestScheduler>;
    type BoxedCoreObservableMutRef<'a, Item: 'a, Err> =
      crate::ops::box_it::BoxedCoreObservableMutRef<'a, Item, Err, CustomTestScheduler>;
    type BoxedCoreObservableClone<'a, Item, Err> =
      crate::ops::box_it::BoxedCoreObservableClone<'a, Item, Err, CustomTestScheduler>;
    type BoxedCoreObservableMutRefClone<'a, Item: 'a, Err> =
      crate::ops::box_it::BoxedCoreObservableMutRefClone<'a, Item, Err, CustomTestScheduler>;

    fn from_parts(inner: T, scheduler: CustomTestScheduler) -> CustomContext<T> {
      CustomContext { inner, scheduler }
    }

    fn lift<U>(inner: U) -> CustomContext<U> {
      CustomContext { inner, scheduler: CustomTestScheduler }
    }

    fn scheduler(&self) -> &CustomTestScheduler { &self.scheduler }

    fn inner(&self) -> &T { &self.inner }

    fn inner_mut(&mut self) -> &mut T { &mut self.inner }

    fn transform<U, F>(self, f: F) -> CustomContext<U>
    where
      F: FnOnce(T) -> U,
    {
      CustomContext { inner: f(self.inner), scheduler: self.scheduler }
    }

    fn wrap<U>(&self, inner: U) -> CustomContext<U> {
      CustomContext { inner, scheduler: self.scheduler }
    }

    fn swap<U>(self, new_inner: U) -> (T, CustomContext<U>) {
      (self.inner, CustomContext { inner: new_inner, scheduler: self.scheduler })
    }

    fn into_inner(self) -> Self::Inner { self.inner }

    fn into_parts(self) -> (Self::Inner, Self::Scheduler) { (self.inner, self.scheduler) }
  }

  #[rxrust_macro::test]
  fn test_factory_blanket_impl_with_defaults() {
    // These should compile due to the blanket implementation using default
    // schedulers
    let _local_of = Local::of(1);
    let _shared_of = Shared::of(2);
  }

  #[rxrust_macro::test]
  fn test_factory_blanket_impl_with_custom_scheduler() {
    // This should compile, demonstrating custom scheduler injection
    let _custom_local_of = CustomContext::of("hello");
  }

  #[rxrust_macro::test]
  fn test_merge_observables_factory() {
    use std::{cell::RefCell, rc::Rc};

    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let obs1 = Local::from_iter([1, 2]);
    let obs2 = Local::from_iter([3, 4]);
    let obs3 = Local::from_iter([5, 6]);

    Local::merge_observables([obs1, obs2, obs3]).subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    let merged = result.borrow();
    assert_eq!(merged.len(), 6);
    assert!(merged.contains(&1));
    assert!(merged.contains(&2));
    assert!(merged.contains(&3));
    assert!(merged.contains(&4));
    assert!(merged.contains(&5));
    assert!(merged.contains(&6));
  }

  #[rxrust_macro::test]
  fn test_merge_observables_factory_with_vec() {
    use std::{cell::RefCell, rc::Rc};

    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    // Test with Vec instead of array
    let observables =
      vec![Local::from_iter([1, 2]), Local::from_iter([3, 4]), Local::from_iter([5, 6])];

    Local::merge_observables(observables).subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    assert_eq!(result.borrow().len(), 6);
  }

  #[rxrust_macro::test]
  fn test_concat_observables_factory() {
    use std::{cell::RefCell, rc::Rc};

    let result = Rc::new(RefCell::new(Vec::new()));
    let result_clone = result.clone();

    let obs1 = Local::from_iter([1, 2]);
    let obs2 = Local::from_iter([3, 4]);
    let obs3 = Local::from_iter([5, 6]);

    Local::concat_observables([obs1, obs2, obs3]).subscribe(move |v| {
      result_clone.borrow_mut().push(v);
    });

    // concat should preserve order
    assert_eq!(*result.borrow(), vec![1, 2, 3, 4, 5, 6]);
  }

  #[rxrust_macro::test]
  fn test_merge_observables_factory_empty() {
    use std::{cell::RefCell, rc::Rc};

    let result = Rc::new(RefCell::new(Vec::<i32>::new()));
    let result_clone = result.clone();
    let completed = Rc::new(RefCell::new(false));
    let completed_clone = completed.clone();

    let observables: Vec<Local<FromIter<std::vec::IntoIter<i32>>>> = vec![];

    Local::merge_observables(observables)
      .on_complete(move || *completed_clone.borrow_mut() = true)
      .subscribe(move |v| {
        result_clone.borrow_mut().push(v);
      });

    assert!(result.borrow().is_empty());
    assert!(*completed.borrow());
  }
}
