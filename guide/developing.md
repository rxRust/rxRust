# A small guide for developer

## How To implement a operator type?

- Add a method in `Observable` to construct your operator type.
- Implement `Observable` for your operator type.
- Implement `LocalObservable` for your operator type in order to handle a subscription (upstream) for single-threaded use.
- Implement `SharedObservable` for your operator type in order to handle a subscription (upstream) for multi-threaded use

## How to implement a creation operator?

Creation operators are functions that can be used to create an Observable with some common predefined behavior. `rxRust` already has a generic struct named `ObservableBase<LocalEmitter>` for implementing creation operators.

`ObservableBase<LocalEmitter>` assume those Observables accept a `Subscriber`  to emit values. So an easy way to implement a creation operator is :

- First, implement an `XxxEmitter` type which implements a `LocalEmitter` trait.
- Second, implement the function you want, It should return a concrete type `ObservableBase<XxxEmitter>`.
- Third, like `LocalObservable`, `LocalEmitter` has a thread-safe version named `SharedEmitter`, remember to implement it for your `XxxEmitter`.



