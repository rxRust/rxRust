/// In `rx_rs` almost all extensions consume the upstream. So as usual it's
/// single-chain. Have to use `fork` to fork stream.
/// # Example
/// ```rust ignore
///  # use rx_rs::prelude::*;
///  let o = observable::from_range(0..10);
///  o.subscribe_err(|_| {println!("consume in first")}, |_:&()|{});
///  o.subscribe_err(|_| {println!("consume in second")}, |_:&()|{});
/// ```
/// it will compile failed, complains like this:
/// ```
// 5 |  let o = observable::from_range(0..10);
//   |      - move occurs because `o` has type `rx_rs::observable::from_iter::RangeObservable<'_, i32, ()>`, which does not implement the `Copy` trait
// 6 |  o.subscribe_err(|_| {println!("consume in first")}, |_:&()|{});
//   |  - value moved here
// 7 |  o.subscribe_err(|_| {println!("consume in second")}, |_:&()|{});
//   |  ^ value used here after move
/// ```
/// 
/// use `fork` to resolve it
/// ```rust
///  # use rx_rs::prelude::*;
///  # use rx_rs::ops::Fork;
///  let o = observable::from_range(0..10);
///  o.fork().subscribe_err(|_| {println!("consume in first")}, |_:&()|{});
///  o.fork().subscribe_err(|_| {println!("consume in second")}, |_:&()|{});
/// ```
use crate::prelude::*;

pub trait Fork {
  #[inline]
  fn fork(&self) -> Sink<&Self> { Sink(self) }
}

impl<'a, S: 'a> Fork for S where &'a S: Subscribable<'a> {}

pub struct Sink<S>(S);
impl<T> Clone for Sink<&T> {
  fn clone(&self) -> Self {
    Sink(self.0)
  }
}

impl<'a, Item, Err: 'a, S> ImplSubscribable<'a> for Sink<S>
where
  S: ImplSubscribable<'a, Item = Item, Err = Err>,
{
  type Item = Item;
  type Err = Err;
  type Unsub = S::Unsub;

  #[inline]
  fn subscribe_return_state(
    self,
    next: impl Fn(&Self::Item) -> OState<Self::Err> + 'a,
    error: Option<impl Fn(&Self::Err) + 'a>,
    complete: Option<impl Fn() + 'a>,
  ) -> Self::Unsub {
    self.0.subscribe_return_state(next, error, complete)
  }
}
