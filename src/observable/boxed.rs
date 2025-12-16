//! Boxed Observable types and traits for type erasure
//!
//! This module provides type-erased observable types for use in contexts
//! where heterogeneous observables need to be stored or passed around.

use crate::{
  context::{Context, LocalCtx, SharedCtx},
  observable::{CoreObservable, ObservableType},
  observer::{
    BoxedObserver, BoxedObserverMutRef, BoxedObserverMutRefSend, BoxedObserverSend,
    IntoBoxedObserver,
  },
  subscription::{BoxedSubscription, BoxedSubscriptionSend, IntoBoxedSubscription},
};

// ============================================================================
// DynCoreObservable Trait
// ============================================================================

/// Object-safe observable trait for type erasure.
///
/// This trait enables boxing observables into trait objects. Unlike the
/// previous design that accepted a raw Observer, this trait accepts a full
/// Context to preserve the scheduler across the type erasure boundary.
///
/// The `Ctx` parameter must be a Context type (e.g.,
/// `LocalCtx<BoxedObserver<...>, S>`).
pub trait DynCoreObservable<Ctx>
where
  Ctx: Context,
{
  /// Subscribe with a context containing a boxed observer, returning a boxed
  /// subscription.
  fn dyn_subscribe(self: Box<Self>, ctx: Ctx) -> Ctx::BoxedSubscription;
}

/// Object-safe clone support for type-erased observables.
///
/// This enables `Clone` for boxed observables when (and only when) the
/// underlying observable pipeline is `Clone`.
pub trait DynCoreObservableClone<'a, Ctx>: DynCoreObservable<Ctx>
where
  Ctx: Context + 'a,
{
  /// Clone this observable into a new boxed trait object.
  fn clone_box(&self) -> Box<dyn DynCoreObservableClone<'a, Ctx> + 'a>;
}

// ============================================================================
// DynCoreObservable Blanket Implementation
// ============================================================================

impl<S, Ctx> DynCoreObservable<Ctx> for S
where
  Ctx: Context,
  S: CoreObservable<Ctx, Unsub: IntoBoxedSubscription<Ctx::BoxedSubscription>>,
{
  fn dyn_subscribe(self: Box<Self>, ctx: Ctx) -> Ctx::BoxedSubscription {
    (*self).subscribe(ctx).into_boxed()
  }
}

impl<'a, S, Ctx> DynCoreObservableClone<'a, Ctx> for S
where
  Ctx: Context + 'a,
  S: CoreObservable<Ctx, Unsub: IntoBoxedSubscription<Ctx::BoxedSubscription>> + Clone + 'a,
{
  fn clone_box(&self) -> Box<dyn DynCoreObservableClone<'a, Ctx> + 'a> { Box::new(self.clone()) }
}

impl<'a, Ctx> Clone for Box<dyn DynCoreObservableClone<'a, Ctx> + 'a>
where
  Ctx: Context + 'a,
{
  fn clone(&self) -> Self { self.clone_box() }
}

// ============================================================================
// Type Aliases for Boxed Observable Types
// ============================================================================

/// Type-erased observable for local value observers (no Send).
///
/// The scheduler type `S` is preserved through the type erasure boundary,
/// ensuring that custom schedulers are not lost when boxing observables.
pub type BoxedCoreObservable<'a, Item, Err, S> =
  Box<dyn DynCoreObservable<LocalCtx<BoxedObserver<'a, Item, Err>, S>> + 'a>;

/// Type-erased observable for shared value observers (with Send).
pub type BoxedCoreObservableSend<'a, Item, Err, S> =
  Box<dyn DynCoreObservable<SharedCtx<BoxedObserverSend<'a, Item, Err>, S>> + Send + 'a>;

/// Type-erased observable for local mut-ref observers (no Send).
pub type BoxedCoreObservableMutRef<'a, Item, Err, S> =
  Box<dyn DynCoreObservable<LocalCtx<BoxedObserverMutRef<'a, Item, Err>, S>> + 'a>;

/// Type-erased observable for shared mut-ref observers (with Send).
pub type BoxedCoreObservableMutRefSend<'a, Item, Err, S> =
  Box<dyn DynCoreObservable<SharedCtx<BoxedObserverMutRefSend<'a, Item, Err>, S>> + Send + 'a>;

/// Cloneable type-erased observable for local value observers (no Send).
pub type BoxedCoreObservableClone<'a, Item, Err, S> =
  Box<dyn DynCoreObservableClone<'a, LocalCtx<BoxedObserver<'a, Item, Err>, S>> + 'a>;

/// Cloneable type-erased observable for shared value observers (with Send).
pub type BoxedCoreObservableSendClone<'a, Item, Err, S> =
  Box<dyn DynCoreObservableClone<'a, SharedCtx<BoxedObserverSend<'a, Item, Err>, S>> + Send + 'a>;

/// Cloneable type-erased observable for local mut-ref observers (no Send).
pub type BoxedCoreObservableMutRefClone<'a, Item, Err, S> =
  Box<dyn DynCoreObservableClone<'a, LocalCtx<BoxedObserverMutRef<'a, Item, Err>, S>> + 'a>;

/// Cloneable type-erased observable for shared mut-ref observers (with Send).
pub type BoxedCoreObservableMutRefSendClone<'a, Item, Err, S> = Box<
  dyn DynCoreObservableClone<'a, SharedCtx<BoxedObserverMutRefSend<'a, Item, Err>, S>> + Send + 'a,
>;

// ============================================================================
// IntoBoxedCoreObservable Trait
// ============================================================================

/// Trait for converting observables into boxed observables.
///
/// Different observer types (value/mut ref, local/shared) are handled via
/// the `Target` type parameter, not separate traits.
pub trait IntoBoxedCoreObservable<Target> {
  /// Convert this observable into a boxed observable.
  fn into_boxed(self) -> Target;
}

// ============================================================================
// IntoBoxedCoreObservable Implementations
// ============================================================================

impl<'a, Item, Err, S, Src> IntoBoxedCoreObservable<BoxedCoreObservable<'a, Item, Err, S>> for Src
where
  S: Clone + Default + 'a,
  Src: DynCoreObservable<LocalCtx<BoxedObserver<'a, Item, Err>, S>> + 'a,
{
  fn into_boxed(self) -> BoxedCoreObservable<'a, Item, Err, S> { Box::new(self) }
}

impl<'a, Item, Err, S, Src> IntoBoxedCoreObservable<BoxedCoreObservableSend<'a, Item, Err, S>>
  for Src
where
  S: Clone + Default + 'a,
  Src: DynCoreObservable<SharedCtx<BoxedObserverSend<'a, Item, Err>, S>> + Send + 'a,
{
  fn into_boxed(self) -> BoxedCoreObservableSend<'a, Item, Err, S> { Box::new(self) }
}

impl<'a, Item: 'a, Err: 'a, S, Src>
  IntoBoxedCoreObservable<BoxedCoreObservableClone<'a, Item, Err, S>> for Src
where
  S: Clone + Default + 'a,
  Src: DynCoreObservableClone<'a, LocalCtx<BoxedObserver<'a, Item, Err>, S>> + 'a,
{
  fn into_boxed(self) -> BoxedCoreObservableClone<'a, Item, Err, S> { Box::new(self) }
}

impl<'a, Item: 'a, Err: 'a, S, Src>
  IntoBoxedCoreObservable<BoxedCoreObservableSendClone<'a, Item, Err, S>> for Src
where
  S: Clone + Default + 'a,
  Src: DynCoreObservableClone<'a, SharedCtx<BoxedObserverSend<'a, Item, Err>, S>> + Send + 'a,
{
  fn into_boxed(self) -> BoxedCoreObservableSendClone<'a, Item, Err, S> { Box::new(self) }
}

#[allow(coherence_leak_check)]
impl<'a, Item: 'a, Err, S, Src> IntoBoxedCoreObservable<BoxedCoreObservableMutRef<'a, Item, Err, S>>
  for Src
where
  S: Clone + Default + 'a,
  Src: DynCoreObservable<LocalCtx<BoxedObserverMutRef<'a, Item, Err>, S>> + 'a,
{
  fn into_boxed(self) -> BoxedCoreObservableMutRef<'a, Item, Err, S> { Box::new(self) }
}

#[allow(coherence_leak_check)]
impl<'a, Item: 'a, Err: 'a, S, Src>
  IntoBoxedCoreObservable<BoxedCoreObservableMutRefClone<'a, Item, Err, S>> for Src
where
  S: Clone + Default + 'a,
  Src: DynCoreObservableClone<'a, LocalCtx<BoxedObserverMutRef<'a, Item, Err>, S>> + 'a,
{
  fn into_boxed(self) -> BoxedCoreObservableMutRefClone<'a, Item, Err, S> { Box::new(self) }
}

#[allow(coherence_leak_check)]
impl<'a, Item: 'a, Err, S, Src>
  IntoBoxedCoreObservable<BoxedCoreObservableMutRefSend<'a, Item, Err, S>> for Src
where
  S: Clone + Default + 'a,
  Src: DynCoreObservable<SharedCtx<BoxedObserverMutRefSend<'a, Item, Err>, S>> + Send + 'a,
{
  fn into_boxed(self) -> BoxedCoreObservableMutRefSend<'a, Item, Err, S> { Box::new(self) }
}

#[allow(coherence_leak_check)]
impl<'a, Item: 'a, Err: 'a, S, Src>
  IntoBoxedCoreObservable<BoxedCoreObservableMutRefSendClone<'a, Item, Err, S>> for Src
where
  S: Clone + Default + 'a,
  Src: DynCoreObservableClone<'a, SharedCtx<BoxedObserverMutRefSend<'a, Item, Err>, S>> + Send + 'a,
{
  fn into_boxed(self) -> BoxedCoreObservableMutRefSendClone<'a, Item, Err, S> { Box::new(self) }
}

// ============================================================================
// ObservableType Implementations for Boxed Types
// ============================================================================

// Local value observable
impl<'a, Item, Err, S> ObservableType for BoxedCoreObservable<'a, Item, Err, S> {
  type Item<'m>
    = Item
  where
    Self: 'm;
  type Err = Err;
}

// Local value observable (cloneable)
impl<'a, Item, Err, S> ObservableType for BoxedCoreObservableClone<'a, Item, Err, S> {
  type Item<'m>
    = Item
  where
    Self: 'm;
  type Err = Err;
}

// Shared value observable
impl<'a, Item, Err, S> ObservableType for BoxedCoreObservableSend<'a, Item, Err, S> {
  type Item<'m>
    = Item
  where
    Self: 'm;
  type Err = Err;
}

// Shared value observable (cloneable)
impl<'a, Item, Err, S> ObservableType for BoxedCoreObservableSendClone<'a, Item, Err, S> {
  type Item<'m>
    = Item
  where
    Self: 'm;
  type Err = Err;
}

// Local mut ref observable
#[allow(coherence_leak_check)]
impl<'a, Item: 'a, Err, S> ObservableType for BoxedCoreObservableMutRef<'a, Item, Err, S> {
  type Item<'m>
    = &'m mut Item
  where
    Self: 'm;
  type Err = Err;
}

// Local mut ref observable (cloneable)
#[allow(coherence_leak_check)]
impl<'a, Item: 'a, Err, S> ObservableType for BoxedCoreObservableMutRefClone<'a, Item, Err, S> {
  type Item<'m>
    = &'m mut Item
  where
    Self: 'm;
  type Err = Err;
}

// Shared mut ref observable
#[allow(coherence_leak_check)]
impl<'a, Item: 'a, Err, S> ObservableType for BoxedCoreObservableMutRefSend<'a, Item, Err, S> {
  type Item<'m>
    = &'m mut Item
  where
    Self: 'm;
  type Err = Err;
}

// Shared mut ref observable (cloneable)
#[allow(coherence_leak_check)]
impl<'a, Item: 'a, Err, S> ObservableType for BoxedCoreObservableMutRefSendClone<'a, Item, Err, S> {
  type Item<'m>
    = &'m mut Item
  where
    Self: 'm;
  type Err = Err;
}

// ============================================================================
// CoreObservable Implementations for Boxed Types
// ============================================================================

// CoreObservable for Local value observable
impl<'a, Item, Err, S, O> CoreObservable<LocalCtx<O, S>> for BoxedCoreObservable<'a, Item, Err, S>
where
  S: Clone + Default,
  O: IntoBoxedObserver<BoxedObserver<'a, Item, Err>>,
{
  type Unsub = BoxedSubscription;

  fn subscribe(self, context: LocalCtx<O, S>) -> Self::Unsub {
    let ctx = context.transform(|o| o.into_boxed());
    self.dyn_subscribe(ctx)
  }
}

// CoreObservable for Local value observable (cloneable)
impl<'a, Item, Err, S, O> CoreObservable<LocalCtx<O, S>>
  for BoxedCoreObservableClone<'a, Item, Err, S>
where
  S: Clone + Default,
  O: IntoBoxedObserver<BoxedObserver<'a, Item, Err>>,
{
  type Unsub = BoxedSubscription;

  fn subscribe(self, context: LocalCtx<O, S>) -> Self::Unsub {
    let ctx = context.transform(|o| o.into_boxed());
    self.dyn_subscribe(ctx)
  }
}

// CoreObservable for Shared value observable
impl<'a, Item, Err, S, O> CoreObservable<SharedCtx<O, S>>
  for BoxedCoreObservableSend<'a, Item, Err, S>
where
  S: Clone + Default,
  O: IntoBoxedObserver<BoxedObserverSend<'a, Item, Err>>,
{
  type Unsub = BoxedSubscriptionSend;

  fn subscribe(self, context: SharedCtx<O, S>) -> Self::Unsub {
    let ctx = context.transform(|o| o.into_boxed());
    self.dyn_subscribe(ctx)
  }
}

// CoreObservable for Shared value observable (cloneable)
impl<'a, Item, Err, S, O> CoreObservable<SharedCtx<O, S>>
  for BoxedCoreObservableSendClone<'a, Item, Err, S>
where
  S: Clone + Default,
  O: IntoBoxedObserver<BoxedObserverSend<'a, Item, Err>>,
{
  type Unsub = BoxedSubscriptionSend;

  fn subscribe(self, context: SharedCtx<O, S>) -> Self::Unsub {
    let ctx = context.transform(|o| o.into_boxed());
    self.dyn_subscribe(ctx)
  }
}

// CoreObservable for Local mut ref observable
#[allow(coherence_leak_check)]
impl<'a, Item: 'a, Err, S, O> CoreObservable<LocalCtx<O, S>>
  for BoxedCoreObservableMutRef<'a, Item, Err, S>
where
  S: Clone + Default,
  O: IntoBoxedObserver<BoxedObserverMutRef<'a, Item, Err>>,
{
  type Unsub = BoxedSubscription;

  fn subscribe(self, context: LocalCtx<O, S>) -> Self::Unsub {
    let ctx = context.transform(|o| o.into_boxed());
    self.dyn_subscribe(ctx)
  }
}

// CoreObservable for Local mut ref observable (cloneable)
#[allow(coherence_leak_check)]
impl<'a, Item: 'a, Err, S, O> CoreObservable<LocalCtx<O, S>>
  for BoxedCoreObservableMutRefClone<'a, Item, Err, S>
where
  S: Clone + Default,
  O: IntoBoxedObserver<BoxedObserverMutRef<'a, Item, Err>>,
{
  type Unsub = BoxedSubscription;

  fn subscribe(self, context: LocalCtx<O, S>) -> Self::Unsub {
    let ctx = context.transform(|o| o.into_boxed());
    self.dyn_subscribe(ctx)
  }
}

// CoreObservable for Shared mut ref observable
#[allow(coherence_leak_check)]
impl<'a, Item: 'a, Err, S, O> CoreObservable<SharedCtx<O, S>>
  for BoxedCoreObservableMutRefSend<'a, Item, Err, S>
where
  S: Clone + Default,
  O: IntoBoxedObserver<BoxedObserverMutRefSend<'a, Item, Err>>,
{
  type Unsub = BoxedSubscriptionSend;

  fn subscribe(self, context: SharedCtx<O, S>) -> Self::Unsub {
    let ctx = context.transform(|o| o.into_boxed());
    self.dyn_subscribe(ctx)
  }
}

// CoreObservable for Shared mut ref observable (cloneable)
#[allow(coherence_leak_check)]
impl<'a, Item: 'a, Err, S, O> CoreObservable<SharedCtx<O, S>>
  for BoxedCoreObservableMutRefSendClone<'a, Item, Err, S>
where
  S: Clone + Default,
  O: IntoBoxedObserver<BoxedObserverMutRefSend<'a, Item, Err>>,
{
  type Unsub = BoxedSubscriptionSend;

  fn subscribe(self, context: SharedCtx<O, S>) -> Self::Unsub {
    let ctx = context.transform(|o| o.into_boxed());
    self.dyn_subscribe(ctx)
  }
}

// ============================================================================
// Type Aliases for Default Schedulers
// ============================================================================

#[cfg(feature = "scheduler")]
use crate::scheduler::{LocalScheduler, SharedScheduler};

#[cfg(feature = "scheduler")]
/// A type-erased Observable (Value) running in a Local context.
/// Use `Local::of(1).box_it()` to create.
pub type LocalBoxedObservable<'a, Item, Err = ()> =
  crate::context::Local<BoxedCoreObservable<'a, Item, Err, LocalScheduler>>;

#[cfg(feature = "scheduler")]
/// A type-erased Observable (Value) running in a Shared context.
/// Use `Shared::of(1).box_it()` to create.
pub type SharedBoxedObservable<'a, Item, Err = ()> =
  crate::context::Shared<BoxedCoreObservableSend<'a, Item, Err, SharedScheduler>>;

#[cfg(feature = "scheduler")]
/// A type-erased Observable (MutRef) running in a Local context.
/// Use `Local::subject_mut_ref::<Item, Err>().box_it_mut_ref()` to create.
pub type LocalBoxedObservableMutRef<'a, Item, Err = ()> =
  crate::context::Local<BoxedCoreObservableMutRef<'a, Item, Err, LocalScheduler>>;

#[cfg(feature = "scheduler")]
/// A type-erased Observable (MutRef) running in a Shared context.
/// Use `Shared::subject_mut_ref::<Item, Err>().box_it_mut_ref()` to create.
pub type SharedBoxedObservableMutRef<'a, Item, Err = ()> =
  crate::context::Shared<BoxedCoreObservableMutRefSend<'a, Item, Err, SharedScheduler>>;

#[cfg(feature = "scheduler")]
/// A cloneable type-erased Observable (Value) running in a Local context.
/// Use `Local::of(1).box_it_clone()` to create.
pub type LocalBoxedObservableClone<'a, Item, Err = ()> =
  crate::context::Local<BoxedCoreObservableClone<'a, Item, Err, LocalScheduler>>;

#[cfg(feature = "scheduler")]
/// A cloneable type-erased Observable (Value) running in a Shared context.
/// Use `Shared::of(1).box_it_clone()` to create.
pub type SharedBoxedObservableClone<'a, Item, Err = ()> =
  crate::context::Shared<BoxedCoreObservableSendClone<'a, Item, Err, SharedScheduler>>;

#[cfg(feature = "scheduler")]
/// A cloneable type-erased Observable (MutRef) running in a Local context.
/// Use `Local::of(1).box_it_mut_ref_clone()` to create.
pub type LocalBoxedObservableMutRefClone<'a, Item, Err = ()> =
  crate::context::Local<BoxedCoreObservableMutRefClone<'a, Item, Err, LocalScheduler>>;

#[cfg(feature = "scheduler")]
/// A cloneable type-erased Observable (MutRef) running in a Shared context.
/// Use `Shared::of(1).box_it_mut_ref_clone()` to create.
pub type SharedBoxedObservableMutRefClone<'a, Item, Err = ()> =
  crate::context::Shared<BoxedCoreObservableMutRefSendClone<'a, Item, Err, SharedScheduler>>;
