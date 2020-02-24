# A small guide for developer

## To implement a operator tip?

- Add a method in `Observable` to construct your operator type.
- Implement `Observable` for your operator type.
- implement `LocalObservable` for your operator type to subscribe upstream and not care thread safe.
- implement `SharedObservable` for your operator type to subscribe upstream and keep thread safe.

## How to implement a creation operator?

Creation operators are functions that can be used to create an Observable with some common predefined behavior. `rxRust` already have a generic struct named ObservableBase<LocalEmitter> for implement creation operators.

ObservableBase<LocalEmitter> assume those Observables are accept a `Subscriber`  to emit values. So a easy way to implement a creation operator is :

- first, implement a `XxxEmitter` type which implement a `LocalEmitter` trait.
- second, implement the function you want, this function just return a concrete type `ObservableBase<XxxEmitter>`.
- third, like `LocalObservable`, `LocalEmitter` has thread safe version named `SharedEmitter`, remember implement it for your `XxxEmitter`.



