/// In `rxrust` almost all extensions consume the upstream. So as usual it's
/// single-chain. Have to use `multicast` and `fork` to fork stream.
/// # Example
/// ```rust ignore
///  # use rxrust::{prelude::*, subscribable::Subscribable};
///  let o = observable::from_range(0..10);
///  o.subscribe_err(|_| {println!("consume in first")}, |_:&()|{});
///  o.subscribe_err(|_| {println!("consume in second")}, |_:&()|{});
/// ```
/// it will compile failed, complains like this:
/// ```
// 5 |  let o = observable::from_range(0..10);
//   |      - move occurs because `o` has type `rxrust::observable::Observable`,
//   |        which does not implement the `Copy` trait
// 6 |  o.subscribe_err(|_| {println!("consume in first")}, |_:&()|{});
//   |  - value moved here
// 7 |  o.subscribe_err(|_| {println!("consume in second")}, |_:&()|{});
//   |  ^ value used here after move
/// ```

/// use `multicast` convert a single-chain stream to one-multi stream.  Then use
/// `fork` to fork a new stream.
/// ```rust
///  # use rxrust::{prelude::*, subscribable::Subscribable};
///  # use rxrust::ops::Fork;
///  let o = observable::from_range(0..10).multicast();
///  o.fork().subscribe_err(|_| {println!("consume in first")}, |_:&()|{});
///  o.fork().subscribe_err(|_| {println!("consume in second")}, |_:&()|{});
/// ```
use crate::subscribable::*;

pub trait Multicast: RawSubscribable {
  type Output: Fork<Item = Self::Item, Err = Self::Err>;
  fn multicast(self) -> Self::Output;
}

pub trait Fork: RawSubscribable {
  type Output: RawSubscribable<Item = Self::Item, Err = Self::Err>;
  fn fork(&self) -> Self::Output;
}

pub trait BoxFork {
  type Item;
  type Err;
  fn box_fork(
    &self,
  ) -> Box<dyn SubscribableByBox<Item = Self::Item, Err = Self::Err>>;
}

pub trait FnPtrFork {
  type Item;
  type Err;
  fn fn_ptr_fork(
    &self,
  ) -> Box<dyn SubscribableByFnPtr<Item = Self::Item, Err = Self::Err>>;
}

impl<S> BoxFork for S
where
  S: Fork,
  S::Output: 'static,
  S::Item: 'static,
  S::Err: 'static,
{
  type Item = S::Item;
  type Err = S::Err;
  fn box_fork(
    &self,
  ) -> Box<dyn SubscribableByBox<Item = Self::Item, Err = Self::Err>> {
    Box::new(self.fork())
  }
}

impl<S> FnPtrFork for S
where
  S: Fork,
  S::Output: 'static,
  S::Item: 'static,
  S::Err: 'static,
{
  type Item = S::Item;
  type Err = S::Err;
  fn fn_ptr_fork(
    &self,
  ) -> Box<dyn SubscribableByFnPtr<Item = Self::Item, Err = Self::Err>> {
    Box::new(self.fork())
  }
}
