# rx_rs: Reactive Extensions for Rust

rx_rs ia a Rust implementation of Reactive Extensions. Which is almost zero cost abstraction except the Subject have to box the first closure of a stream.

## Example 

```rust
use rx_rs::{ ops::{ Filter, Merge }, prelude::*};

let numbers = Subject::new();
// crate a even stream by filter
let even = numbers.clone().filter(|v| *v % 2 == 0);
// crate an odd stream by filter
let odd = numbers.clone().filter(|v| *v % 2 != 0);

// merge odd and even stream again
let merged = even.merge(odd);

// attach observers
let subscription = merged
  .subscribe(|v| print!("{} ", v))
  .on_error(|e| println!("Error because of: {}", e));

// shot numbers
(0..10).into_iter().for_each(|v| {
    numbers.next(&v);
});
// "0 1 2 3 4 5 6 7 8 9" will be printed.

numbers.error(&"just trigger an error.");
// will print: "Error because of: just trigger an error."

```

## Runtime error propagating

 In rx_rs, every extension has two version method. One version is use when no runtime error will be propagated. This version receive an normal closure. The other is use when when will propagating runtime error, named `xxx_with_err`, and receive an closure that return an `Result` type, to detect if an runtime error occur. For example:

```rust
use rx_rs::{ops::{ Map, MapWithErr }, prelude::*};


// normal version
// double a number
let subject = Subject::new();
subject.clone()
  .map(|i| 2 * i)
  .subscribe(|v| print!("{} | ", v));

// runtime error version
// only double a even number. otherwise throw an error.
subject.clone()
  .map_with_err(|i| {
    if i % 2 == 0 {Ok(i*2)}
    else {Err("odd number should never be pass to here")}
  })
  .subscribe(|v| print!("{} | ", v))
  .on_error(|err|{println!("{} | ", err)});

subject.next(&0);
subject.next(&1);
// normal version will print `0 | ` and `2 |`, 
// runtime error version will print `0 | ` and `odd number should never be pass to here | "
// this example print "0 | 0 | 2 | odd number should never be pass to here | "
```

