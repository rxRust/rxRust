# rxrust: Reactive Extensions for Rust

rxrust ia a Rust implementation of Reactive Extensions. Which is almost zero cost abstraction except the Subject have to box the first closure of a stream. [Documents](https://docs.rs/rxrust)

## Example 

```rust
use rxrust::{
  ops::{ Filter, Merge, Fork }, prelude::*, 
};

let mut numbers = observable::from_iter!(0..10);
// crate a even stream by filter
let even = numbers.fork().filter(|v| *v % 2 == 0);
// crate an odd stream by filter
let odd = numbers.fork().filter(|v| *v % 2 != 0);

// merge odd and even stream again
even.merge(odd).subscribe(|v| print!("{} ", v, ));
// "0 1 2 3 4 5 6 7 8 9" will be printed.

```

## Fork Stream

In `rxrust` almost all extensions consume the upstream. So in general it is unicast. So when you try to subscribe a stream twice, the compiler will complain. 

```rust ignore
 # use rxrust::prelude::*;
 let o = observable::from_iter!(0..10);
 o.subscribe(|_| {println!("consume in first")});
 o.subscribe(|_| {println!("consume in second")});
```

In this case, we can use `multicast` convert an unicast stream to a multicast stream. A multicast stream is a stream that implements `Fork` trait, let you can fork stream from it. Subject is an multicast stream default, so you can direct fork it. 

```rust
 # use rxrust::prelude::*;
 # use rxrust::ops::Fork;
 let o = observable::from_iter!(0..10);
 o.fork().subscribe(|_| {println!("consume in first")});
 o.fork().subscribe(|_| {println!("consume in second")});
```

## Scheduler

```rust 
use rxrust::prelude::*;
use rxrust::{ops::{ ObserveOn, SubscribeOn, Map }, scheduler::Schedulers };

observable::from_iter!(0..10)
  .subscribe_on(Schedulers::NewThread)
  .map(|v| *v*2)
  .observe_on(Schedulers::NewThread)
  .subscribe(|v| {println!("{},", v)});
```

## Converts from a Future

just use `observable::from_future!` to convert a `Future` to an observable sequence.

```rust
use rxrust::prelude::*;
use futures::future;

observable::from_future!(future::ready(1))
  .subscribe(move |v| println!("subscribed with {}", v));

// because all future in rxrust are execute async, so we wait a second to see
// the print, no need to use this line in your code.
std::thread::sleep(std::time::Duration::new(1, 0));
```

A `from_future_with_err` macro also provided to propagating error from `Future`.
