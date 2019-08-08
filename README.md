# rxrust: Reactive Extensions for Rust

rxrust ia a Rust implementation of Reactive Extensions. Which is almost zero cost abstraction except the Subject have to box the first closure of a stream.

## Example 

```rust
use rxrust::{
  ops::{ Filter, Merge, Fork }, prelude::*, 
};

let mut numbers = observable::from_range(0..10).multicast();
// crate a even stream by filter
let even = numbers.fork().filter(|v| *v % 2 == 0);
// crate an odd stream by filter
let odd = numbers.fork().filter(|v| *v % 2 != 0);

// merge odd and even stream again
even.merge(odd).subscribe(|v| print!("{} ", v, ));
// "0 1 2 3 4 5 6 7 8 9" will be printed.

```

### Fork Stream

In `rxrust` almost all extensions consume the upstream. So in general it is unicast. So when you try to subscribe a stream twice, the compiler will complain. 

```rust ignore
 # use rxrust::prelude::*;
 let o = observable::from_range(0..10);
 o.subscribe(|_| {println!("consume in first")});
 o.subscribe(|_| {println!("consume in second")});
```

In this case, we can use `multicast` convert an unicast stream to a multicast stream. A multicast stream is a stream that implements `Fork` trait, let you can fork stream from it. Subject is an multicast stream default, so you can direct fork it. 

```rust
 # use rxrust::prelude::*;
 # use rxrust::ops::Fork;
 let o = observable::from_range(0..10).multicast();
 o.fork().subscribe(|_| {println!("consume in first")});
 o.fork().subscribe(|_| {println!("consume in second")});
```

### Scheduler

For now, only a new thread scheduler has been implemented. 

```rust 
use rxrust::prelude::*;
use rxrust::{ops::{ ObserveOn, SubscribeOn, Map }, scheduler };

observable::from_range(0..10)
  .subscribe_on(scheduler::new_thread())
  .map(|v| *v*2)
  .observe_on(scheduler::new_thread())
  .subscribe(|v| {println!("{},", v)});
```

## Runtime error propagating

 In rxrust, almost every extension which accept closure as argument has two version method. One version is use when no runtime error will be propagated. This version receive an normal closure. The other is use when when you want propagating runtime error, named `xxx_with_err`, and receive an closure that return an `Result` type, to detect if an runtime error occur. For example:

```rust
use rxrust::{ops::{ Map, MapWithErr }, prelude::*};

// normal version
// double a number
let subject = Subject::new();
subject.fork()
  .map(|i| 2 * i)
  .subscribe(|v| print!("{} | ", v));

// runtime error version
// only double a even number. otherwise throw an error.
subject.fork()
  .map_with_err(|i| {
    if i % 2 == 0 {Ok(i*2)}
    else {Err("odd number should never be pass to here")}
  })
  .subscribe_err(|v| print!("{} | ", v), |err|{println!("{} | ", err)});

subject.next(&0);
subject.next(&1);
// normal version will print `0 | ` and `2 |`, 
// runtime error version will print `0 | ` and `odd number should never be pass to here | "
// this example print "0 | 0 | 2 | odd number should never be pass to here | "
```

