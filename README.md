# RxRust: a zero cost Rust implementation of Reactive Extensions
[![](https://docs.rs/rxrust/badge.svg)](https://docs.rs/rxrust/)
[![codecov](https://codecov.io/gh/rxRust/rxRust/branch/master/graph/badge.svg)](https://codecov.io/gh/rxRust/rxRust)
![](https://github.com/M-Adoo/rxRust/workflows/test/badge.svg)
[![](https://img.shields.io/crates/v/rxrust.svg)](https://crates.io/crates/rxrust)
[![](https://img.shields.io/crates/d/rxrust.svg)](https://crates.io/crates/rxrust)

## Usage
Add this to your Cargo.toml:

```ignore
[dependencies]
rxrust = "0.7.1";
```

## Example 

```rust
use rxrust::{
  ops::{ Filter, Merge, Fork }, prelude::*, 
};

let mut numbers = observable::from_iter(0..10);
// crate a even stream by filter
let even = numbers.fork().filter(|v| v % 2 == 0);
// crate an odd stream by filter
let odd = numbers.fork().filter(|v| v % 2 != 0);

// merge odd and even stream again
even.merge(odd).subscribe(|v| print!("{} ", v, ));
// "0 1 2 3 4 5 6 7 8 9" will be printed.

```

## Fork Stream

In `rxrust` almost all extensions consume the upstream. So when you try to subscribe a stream twice, the compiler will complain. 

```rust ignore
 # use rxrust::prelude::*;
 let o = observable::from_iter(0..10);
 o.subscribe(|_| {println!("consume in first")});
 o.subscribe(|_| {println!("consume in second")});
```

In this case, we can use `Fork` to fork a stream. In general, `Fork` has same mean with `Clone`, this will not change a cold stream to hot stream. If you want convert a stream from  unicast to multicast, from **cold** to **hot** use `Publish` and `RefCount`.

```rust
 # use rxrust::prelude::*;
 # use rxrust::ops::Fork;
 let o = observable::from_iter(0..10);
 o.fork().subscribe(|_| {println!("consume in first")});
 o.fork().subscribe(|_| {println!("consume in second")});
```

## Scheduler

```rust 
use rxrust::prelude::*;
use rxrust::{ops::{ ObserveOn, SubscribeOn, Map }, scheduler::Schedulers };

observable::from_iter(0..10)
  .subscribe_on(Schedulers::NewThread)
  .map(|v| v*2)
  .observe_on(Schedulers::NewThread)
  .subscribe(|v| {println!("{},", v)});
```

## Converts from a Future

just use `observable::from_future` to convert a `Future` to an observable sequence.

```rust
use rxrust::prelude::*;
use futures::future;

observable::from_future(future::ready(1))
  .subscribe(move |v| println!("subscribed with {}", v));

// because all future in rxrust are execute async, so we wait a second to see
// the print, no need to use this line in your code.
std::thread::sleep(std::time::Duration::new(1, 0));
```

A `from_future_with_err` function also provided to propagating error from `Future`.

## Missing Features List
See [missing features](missing_features.md) to know what rxRust not have for now.

## All contributions are welcome

We are looking for contributors! Feel free to open issues for asking questions, suggesting features or other things!

Help and contributions can be any of the following:

- use the project and report issues to the project issues page
- documentation and README enhancement (VERY important)
- continuous improvement in a ci Pipeline
- implement any unimplemented operator, remember to create a pull request before you start your code, so other people know you are work on it.