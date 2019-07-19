/// Convert a Subscribable to Subject. This is different to `fork`. `fork` only
/// fork a new stream from the origin, it's a lazy operator, and `multicast`
/// will subscribe origin stream immediately and return an subject. Normally use
/// `fork` for a stream from Observable, use multicast for Subject.
///
/// # Example
///
/// ```
/// # use rx_rs::{ops::{ Multicast, Fork, Filter }, prelude::*};
///
/// let even = Subject::<'_, _, ()>::new().filter(|v| *v % 2 == 0);
/// // convert numbers to Subject
/// let multicast = even.multicast();
/// // now you can subscribe many times on it.
/// multicast.clone().subscribe(|v| println!("first stream get {}", v));
/// multicast.clone().subscribe(|v| println!("second stream get {}", v));
/// multicast.clone().subscribe(|v| println!("third stream get {}", v));
/// // or directly pass items from multicast.
/// multicast.next(&1);
/// ```
///
/// **Remember multicast will immediately subscribe on the source stream.  So
/// multicast may consume the sync upstream data.**
/// ```rust
/// # use rx_rs::{ops::{ Multicast, Fork, Filter }, prelude::*};
/// observable::of(0)
///   .multicast()
///   .subscribe(|v| println!("{}", v));
///
/// // This will print nothing, because a subscribe is called inner multicast
/// // and consumed the data.  
/// ```
use crate::prelude::*;

pub trait Multicast<'a> {
  type Item;
  type Err;
  fn multicast(self) -> Subject<'a, Self::Item, Self::Err>;
}

impl<'a, S: 'a> Multicast<'a> for S
where
  S: ImplSubscribable<'a>,
{
  type Item = S::Item;
  type Err = S::Err;
  fn multicast(self) -> Subject<'a, Self::Item, Self::Err> {
    let broadcast = Subject::new();
    let for_next = broadcast.clone();
    let for_complete = broadcast.clone();
    let for_err = broadcast.clone();
    self.subscribe_err_complete(
      move |v| {
        for_next.next(v);
      },
      move |err: &S::Err| {
        for_err.clone().error(err);
      },
      move || {
        for_complete.clone().complete();
      },
    );

    broadcast
  }
}
