# rxRust: a Rust implementation of Reactive Extensions
[![](https://docs.rs/rxrust/badge.svg)](https://docs.rs/rxrust/)
[![codecov](https://codecov.io/gh/rxRust/rxRust/branch/master/graph/badge.svg)](https://codecov.io/gh/rxRust/rxRust)
![](https://github.com/rxRust/rxRust/workflows/test/badge.svg)
[![](https://img.shields.io/crates/v/rxrust.svg)](https://crates.io/crates/rxrust)
[![](https://img.shields.io/crates/d/rxrust.svg)](https://crates.io/crates/rxrust)

## Usage

`1.0.x` version requires Rust nightly before GAT stable, `0.15.0` version works with Rust stable.

Add this to your Cargo.toml:

```toml
[dependencies]
rxrust = "1.0.0-alpha.2"
```

## Example 

```rust
use rxrust:: prelude::*;

let mut numbers = observable::from_iter(0..10);
// create an even stream by filter
let even = numbers.clone().filter(|v| v % 2 == 0);
// create an odd stream by filter
let odd = numbers.clone().filter(|v| v % 2 != 0);

// merge odd and even stream again
even.merge(odd).subscribe(|v| print!("{} ", v, ));
// "0 1 2 3 4 5 6 7 8 9" will be printed.

```

## Clone Stream

In `rxrust` almost all extensions consume the upstream. So when you try to subscribe a stream twice, the compiler will complain. 

```rust ignore
 # use rxrust::prelude::*;
 let o = observable::from_iter(0..10);
 o.subscribe(|_| { println!("consume in first")} );
 o.subscribe(|_| { println!("consume in second")} );
```

In this case, we must clone the stream.

```rust
 # use rxrust::prelude::*;
 let o = observable::from_iter(0..10);
 o.clone().subscribe(|_| {println!("consume in first")});
 o.clone().subscribe(|_| {println!("consume in second")});
```

## Scheduler

`rxrust` use the runtime of the `Future` as the scheduler, `LocalPool` and `ThreadPool` in `futures::executor` can be used as schedulers directly, and `tokio::runtime::Runtime` also supported, but need enable the feature `futures-scheduler`. Across `LocalScheduler` and `SharedScheduler` to implement custom `Scheduler`.

```rust 
use rxrust::prelude::*;
use futures::executor::ThreadPool;

let pool_scheduler = ThreadPool::new().unwrap();
observable::from_iter(0..10)
  .subscribe_on(pool_scheduler.clone())
  .map(|v| v*2)
  .into_shared()
  .observe_on(pool_scheduler)
  .into_shared()
  .subscribe(|v| {println!("{},", v)});
```

Also, `rxrust` supports WebAssembly by enabling the feature `wasm-scheduler` and using the crate `wasm-bindgen`. Simple example is [here](https://github.com/utilForever/rxrust-with-wasm). Note that `wasm-scheduler` only supports `LocalScheduler`.

## Converts from a Future

Just use `observable::from_future` to convert a `Future` to an observable sequence.

```rust
use rxrust::prelude::*;
use futures::{ future, executor::LocalPool };

let mut local_scheduler = LocalPool::new();
observable::from_future(future::ready(1), local_scheduler.spawner())
  .subscribe(move |v| println!("subscribed with {}", v));

// Wait `LocalPool` finish.
local_scheduler.run();
```

A `from_future_result` function also provided to propagating error from `Future`.

## Missing Features List
See [missing features](missing_features.md) to know what rxRust does not have yet.

## All contributions are welcome

We are looking for contributors! Feel free to open issues for asking questions, suggesting features or other things!

Help and contributions can be any of the following:

- use the project and report issues to the project issues page
- documentation and README enhancement (VERY important)
- continuous improvement in a ci Pipeline
- implement any unimplemented operator, remember to create a pull request before you start your code, so other people know you are work on it.
