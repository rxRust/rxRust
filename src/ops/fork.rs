/// In `rxrust` almost all extensions consume the upstream. So as usual it's
/// single-chain. Have to use `fork` to fork stream.
/// # Example
/// ```rust ignore
///  # use rxrust::prelude::*;
///  let o = observable::from_iter(0..10);
///  o.subscribe_err(|_| {println!("consume in first")}, |_:&()|{});
///  o.subscribe_err(|_| {println!("consume in second")}, |_:&()|{});
/// ```
/// it will compile failed, complains like this:
/// ```shell
// 5 |  let o = observable::from_iter(0..10);
//   |      - move occurs because `o` has type `rxrust::observable::Observable`,
//   |        which does not implement the `Copy` trait
// 6 |  o.subscribe_err(|_| {println!("consume in first")}, |_:&()|{});
//   |  - value moved here
// 7 |  o.subscribe_err(|_| {println!("consume in second")}, |_:&()|{});
//   |  ^ value used here after move
/// ```

/// Use `fork` to fork a new stream.
///
/// ```rust
///  # use rxrust::prelude::*;
///  # use rxrust::ops::Fork;
///  let o = observable::from_iter(0..10);
///  o.fork().subscribe_err(|_| {println!("consume in first")}, |_:&()|{});
///  o.fork().subscribe_err(|_| {println!("consume in second")}, |_:&()|{});
/// ```
/// Note `Fork` will not change a cold stream to hot stream.

pub trait Fork {
  type Output;
  fn fork(&self) -> Self::Output;
}
