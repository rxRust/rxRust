# rx_rs: Reactive Extensions for Rust

rx_rs ia a Rust implementation of Reactive Extensions. Which is almost zero cost abstraction except the Subject have to box the first closure of a stream.

## Example 

```rust
use rx_rs::{ 
  ops::{ Filter, Merge }, Observable, Observer,
  Subject, Subscription 
};

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
    numbers.next(v);
});

// "0 1 2 3 4 5 6 7 8 9" will be print.

// "Error because of: just trigger an error."
numbers.error("just trigger an error.");
```