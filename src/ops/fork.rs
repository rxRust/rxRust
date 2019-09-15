/// In `rxrust` almost all extensions consume the upstream. So as usual it's
/// single-chain. Have to use `multicast` and `fork` to fork stream.
/// # Example
/// ```rust ignore
///  # use rxrust::prelude::*;
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
///  # use rxrust::prelude::*;
///  # use rxrust::ops::Fork;
///  let o = observable::from_range(0..10).multicast();
///  o.fork().subscribe_err(|_| {println!("consume in first")}, |_:&()|{});
///  o.fork().subscribe_err(|_| {println!("consume in second")}, |_:&()|{});
/// ```

pub trait Multicast {
  type Output;
  fn multicast(self) -> Self::Output;
}

pub trait Fork {
  type Output;
  fn fork(&self) -> Self::Output;
}

impl<T> Multicast for T {
  type Output = T;
  #[inline(always)]
  fn multicast(self) -> Self::Output { self }
}

impl<T> Fork for T
where
  T: Clone,
{
  type Output = T;
  #[inline(always)]
  fn fork(&self) -> Self::Output { self.clone() }
}
