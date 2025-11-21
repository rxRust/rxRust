# rxRust: a Rust implementation of Reactive Extensions
[![](https://docs.rs/rxrust/badge.svg)](https://docs.rs/rxrust/)
[![codecov](https://codecov.io/gh/rxRust/rxRust/branch/master/graph/badge.svg)](https://codecov.io/gh/rxRust/rxRust)
![](https://github.com/rxRust/rxRust/workflows/test/badge.svg)
[![](https://img.shields.io/crates/v/rxrust.svg)](https://crates.io/crates/rxrust)
[![](https://img.shields.io/crates/d/rxrust.svg)](https://crates.io/crates/rxrust)

## Usage

Add this to your Cargo.toml:

```toml
[dependencies]
rxrust = "1.0.0-beta.0"
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
// "0 2 4 6 8 1 3 5 7 9" will be printed.

```

## Clone Stream

In `rxrust` almost all extensions consume the upstream. So when you try to subscribe a stream twice, the compiler will complain. 

```rust ignore
 # use rxrust::prelude::*;
 let o = observable::from_iter(0..10);
 o.subscribe(|_| println!("consume in first"));
 o.subscribe(|_| println!("consume in second"));
```

In this case, we must clone the stream.

```rust
 # use rxrust::prelude::*;
 let o = observable::from_iter(0..10);
 o.clone().subscribe(|_| println!("consume in first"));
 o.clone().subscribe(|_| println!("consume in second"));
```

If you want to share the same observable, you can use `Subject`.

## Scheduler

`rxrust` provides a unified scheduler system that works across all platforms (including WebAssembly) using tokio as the foundation. The scheduler system is enabled by the `scheduler` feature. On WebAssembly platforms, `tokio_with_wasm` is used as an adapter to provide tokio-compatible functionality.

Two scheduler types are available:

- `LocalScheduler`: For single-threaded local execution contexts
- `SharedScheduler`: For multi-threaded shared execution contexts

```rust
use rxrust::prelude::*;
use rxrust::scheduler::{LocalScheduler, SharedScheduler};

#[tokio::main(flavor = "multi_thread")]
async fn main() {
  let shared_scheduler = SharedScheduler;

  observable::from_iter(0..10)
    .subscribe_on(shared_scheduler)
    .map(|v| v*2)
    .observe_on_threads(shared_scheduler)
    .subscribe(|v| println!("{},", v));
}

// For local execution, use LocalScheduler with LocalSet
async fn local_example() {
  let local_set = tokio::task::LocalSet::new();
  let _guard = local_set.enter();

  observable::from_iter(0..10)
    .observe_on(LocalScheduler)
    .subscribe(|v| println!("{},", v));

  local_set.await;
}
```

The scheduler system automatically works across all platforms including WebAssembly without requiring additional configuration. For timer-based operations (such as `delay`, `debounce`, `timer`, `interval`), the `timer` feature provides default timing functionality using tokio's timer.

### Custom Scheduler Implementation

You can also implement your own custom scheduler by disabling the `scheduler` feature and implementing the `Scheduler` trait:

```rust
use rxrust::prelude::*;
use rxrust::scheduler::{Scheduler, TaskHandle};
use std::time::Duration;

// Disable default scheduler in Cargo.toml:
// [dependencies]
// rxrust = { version = "1.0.0-beta.0", default-features = false }

// Implement your own scheduler
struct MyCustomScheduler;

impl<T> Scheduler<T> for MyCustomScheduler
where
    T: std::future::Future + Send + 'static,
    T::Output: Send,
{
    fn schedule(&self, task: T, delay: Option<Duration>) -> TaskHandle<T::Output> {
        // Your custom scheduling logic here
        // This could integrate with other async runtimes, thread pools, etc.
        todo!("Implement your scheduling logic")
    }
}
``` 

## Converts from a Future

Just use `observable::from_future` to convert a `Future` to an observable sequence.

```rust
use rxrust::prelude::*;
use rxrust::scheduler::LocalScheduler;

#[tokio::main]
async fn main() {
  let local_set = tokio::task::LocalSet::new();
  let _guard = local_set.enter();

  observable::from_future(std::future::ready(1), LocalScheduler)
    .subscribe(move |v| println!("subscribed with {}", v));

  // Wait for the future to complete.
  local_set.await;
}
```

A `from_future_result` function is also provided to propagate errors from `Future``.

## Missing Features List
See [missing features](missing_features.md) to know what rxRust does not have yet.

## All contributions are welcome

We are looking for contributors! Feel free to open issues for asking questions, suggesting features or other things!

Help and contributions can be any of the following:

- use the project and report issues to the project issues page
- documentation and README enhancement (VERY important)
- continuous improvement in a ci Pipeline
- implement any unimplemented operator, remember to create a pull request before you start your code, so other people know you are working on it.

You can enable the default timer by the `timer` feature, or provide a custom timer implementation by setting the `new_timer_fn` function and removing the `timer` feature.