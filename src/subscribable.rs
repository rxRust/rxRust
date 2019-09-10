mod subscribable_all;
pub use subscribable_all::*;
mod subscribable_err;
pub use subscribable_err::*;
mod subscribable_pure;
pub use subscribable_pure::*;
mod subscribable_comp;
pub use subscribable_comp::*;

pub enum RxValue<T, E> {
  Next(T),
  Err(E),
  Complete,
}

impl<T, E> RxValue<T, E> {
  pub fn as_ref(&self) -> RxValue<&T, &E> {
    match self {
      RxValue::Next(n) => RxValue::Next(&n),
      RxValue::Err(e) => RxValue::Err(&e),
      RxValue::Complete => RxValue::Complete,
    }
  }

  pub fn as_mut(&mut self) -> RxValue<&T, &E> {
    match self {
      RxValue::Next(ref mut n) => RxValue::Next(n),
      RxValue::Err(ref mut e) => RxValue::Err(e),
      RxValue::Complete => RxValue::Complete,
    }
  }
}

impl<T, E> RxValue<&T, &E>
where
  T: Clone,
  E: Clone,
{
  pub fn to_owned(&self) -> RxValue<T, E> {
    match self {
      RxValue::Next(n) => RxValue::Next((*n).clone()),
      RxValue::Err(e) => RxValue::Err((*e).clone()),
      RxValue::Complete => RxValue::Complete,
    }
  }
}

/// `Item` the type of the elements being emitted.
/// `Err`the type of the error may propagating.
pub trait Subscribe<Item, Err> {
  fn run(&self, v: RxValue<&'_ Item, &'_ Err>);
}

impl<Item, Err, T> Subscribe<Item, Err> for T
where
  T: Fn(RxValue<&'_ Item, &'_ Err>),
{
  #[inline(always)]
  fn run(&self, v: RxValue<&'_ Item, &'_ Err>) { self(v) }
}

pub trait IntoSharedSubscribe<Item, Err, Shared>
where
  Shared: Subscribe<Item, Err> + Sync + Send + 'static,
{
  fn to_shared(self) -> Shared;
}

impl<Item, Err, T> IntoSharedSubscribe<Item, Err, T> for T
where
  T: Subscribe<Item, Err> + Send + Sync + 'static,
{
  #[inline(always)]
  fn to_shared(self) -> Self { self }
}

pub trait RawSubscribable<Item, Err, S>
where
  S: Subscribe<Item, Err>,
{
  /// a type implemented [`Subscription`]
  type Unsub;

  fn raw_subscribe(self, subscribe: S) -> Self::Unsub;
}

pub trait IntoSharedSubscribable<
  Item,
  Err,
  S: Subscribe<Item, Err>,
  Shared: RawSubscribable<Item, Err, S> + Sync + Send + 'static,
>
{
  fn to_shared(self) -> Shared;
}

impl<T, S, Item, Err> IntoSharedSubscribable<Item, Err, S, T> for T
where
  S: Subscribe<Item, Err>,
  T: RawSubscribable<Item, Err, S> + Sync + Send + 'static,
{
  #[inline(always)]
  fn to_shared(self) -> T { self }
}
// todo: define a safe RawSubscribable return a Box<Subscription> let
// we can crate a object safety object ref.

// pub trait Subscribable {
//   type Item;
//   type Err;

//   /// Convert a Subscribable to Subject. This is different to [`Fork`]. `fork`
//   /// only fork a new stream from the origin, it's a lazy operator, but
//   /// `into_subject` will subscribe origin stream immediately and return an
//   /// subject.
//   // fn into_subject(self) -> Subject<Self::Item, Self::Err>
//   // where
//   //   Self::Item: 'static,
//   //   Self::Err: 'static;
// }

// impl<S: RawSubscribable> Subscribable for S {
//   type Item = S::Item;
//   type Err = S::Err;

//   }

//   #[inline(always)]
//   fn into_subject(self) -> Subject<Self::Item, Self::Err>
//   where
//     Self::Item: 'static,
//     Self::Err: 'static,
//   {
//     Subject::from_subscribable(self)
//   }
// }
