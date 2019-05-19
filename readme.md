# rx_rs: Reactive Extensions for Rust

rx_rs ia a Rust implementation of Reactive Extensions. Which is zero cost abstractions except the Subject have to box the first closure of a stream.

## Example 

```
use rx_rs::{ops::{Filter, Merge}, Observable, Observer, Subject};

let numbers = Subject::new();
// crate a even stream by filter
let even = numbers.clone().filter(|v| *v % 2 == 0);
// crate an odd stream by filter
let odd = numbers.clone().filter(|v| *v % 2 != 0);

// merge odd and even stream again
let merged = even.merge(odd);

// attach observers
merged.subscribe(|v| println!("{} ", v));
```