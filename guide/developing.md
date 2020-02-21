# A small guide for developer

## To implement a operator tip?

`rxRust` has two base trait named `Observable` and `SharedObservable`, these two trait almost the same. `Observable` is used for local thread, and `SharedObservable` is a thread safe version. So your should remember implement both two for your operator.

## How to implement a creation operator?

Creation operators are functions that can be used to create an Observable with some common predefined behavior. `rxRust` already have a generic struct named ObservableBase<Emitter> for implement creation operators.

ObservableBase<Emitter> assume those Observables are accept a `Subscriber`  to emit values. So a easy way to implement a creation operator is :

- first, implement a `XxxEmitter` type which implement a `Emitter` trait.
- second, implement the function you want, this function just return a concrete type `ObservableBase<XxxEmitter>`.
- third, like `Observable`, `Emitter` has thread safe version named `SharedEmitter`, remember implement it for your `XxxEmitter`.



