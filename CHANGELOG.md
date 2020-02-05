## [Unreleased](https://github.com/rxRust/rxRust/compare/v0.7.3...HEAD)

### Features
- **operator**: add `take_last` operator.

### Refactor

- **observable**: Observable split into many concrete type, not only use a Observable struct to wrap all, every observable creating function has a concrete type.

### Breaking Changes

- **observable**: remove `Observable::new`, and add a same `create` function in `observable` to replace it.
- **observable**: Rename `Observable` to `ObservableFromFn`.

## [0.7.2](https://github.com/rxRust/rxRust/releases/tag/v0.7.2)  (2020-01-09)

### Refactor

- **Subject**: merge four version local subject into same version.

### Breaking Changes

- **Subject**: `Subject::local`,`Subject::local_mut_ref`, `Subject::local_mut_ref_item` and `Subject::local_mut_ref_err` merge into `Subject::local`.

## [0.7.1](https://github.com/rxRust/rxRust/releases/tag/v0.7.1)  (2019-12-12)

**Nothing changed, just fix release package**

## [0.7.0](https://github.com/rxRust/rxRust/releases/tag/v0.7.0)  (2019-12-12)

### Features

- **Subject**: local subject support emit mut ref item.

### Breaking Changes

- **observable**: `LocalConnectableObservable` and `SharedConnectableObservable` has merged into `ConnectableObservable`
- **observable**: remove generic type `Item` and `Err` from `RawSubscribable`, almost not effect user code.

## [0.6.0](https://github.com/rxRust/rxRust/releases/tag/v0.6.0)  (2019-12-07)

### Breaking Changes

- **observer**: `Observer::next` emit items by value instead of reference.
- **operator**: remove `map_return_ref` operator, now `map` cover its use scenes.
- **operator**: remove `filter_map_return_ref`, now `filter_map` cover its use scenes.

## [0.5.0](https://github.com/rxRust/rxRust/releases/tag/v0.5.0)  (2019-11-19)

### Features
- **operator**: add `scan` operator.
- **observable**: add trivial `throw`, `empty`, `never` and `repeat` observables.
- **operator**: add `last` and `last_or` operators.
- **operator**: add `reduce` and `reduce_initial` operators.
- **operator**: add `sum`,`min`,`max`,`count` and `average` math/aggregate operators.
- **operator**: add `filter_map` and `filter_map_return_ref` observables.

### Bug Fixes
- **operator**: fix the compiler complain when `map` operator convert source type to a different one.

### Breaking Changes
- **observable**: macros `of!`, `empty!`, `from_iter!`, `from_future!` and
  `from_future_with_errors!` replaced by functions.

## [0.4.0](https://github.com/rxRust/rxRust/releases/tag/v0.4.0)  (2019-11-07)

### Features
- **observable**: add `ConnectableObservable` to support multicast.
- **operator**: add `throttle_time` operator
- **operator**: add `publish` operator
- **operator**: add `ref_count` operator
- **Subject**: support `Fork` even if `Item` and `Err` not support `Clone`.

### Breaking Changes

**Scheduler**: add a `delay` param for `schedule` method, from 
```
pub trait Scheduler {
  fn schedule<T: Send + Sync + 'static>(
    &self,
    task: impl FnOnce(SharedSubscription, T) + Send + 'static,
    state: T,
  ) -> SharedSubscription;
}
```
to
```
pub trait Scheduler {
  fn schedule<T: Send + 'static>(
    &self,
    task: impl FnOnce(SharedSubscription, T) + Send + 'static,
    delay: Option<Duration>,
    state: T,
  ) -> SharedSubscription;
}
```

## [0.3.0](https://github.com/rxRust/rxRust/releases/tag/v0.3.0)  (2019-10-12)

### Code Refactoring

In `v0.2` we implemented all operators and observable thread safe， so we can pass task across threads by schedulers. In this way, all user provide closure must satisfied `Send + Sync + 'static`, even never use scheduler and multi-thread.

For now, we removed the bounds `Sync`, `Send` and `'static`, and add a new trait `IntoShared`. We always implemented operator for local thread, and implement `IntoShared` for it to convert it to a thread-safe operator.
By default, RxRust always use single thread version to get the best performance, and use `IntoShared` to convert a local object to a thread-safe object if we need pass this object in threads.

**Before**:
```rust
let res = Arc::new(Mutex(0));w
let c_res = res.clone();
observable::of(100).subscribe(|v| { *res.lock().unwrap() = *v });

assert_eq!(*res.lock().unwrap(), 100);
```

**After**:

```rust
let mut res = 0;
observable::of(100).subscribe(|v| { res = *v });

assert_eq!(res, 100);
```

### Breaking Changes

- removed `RxFn` and `RxValue`
- **operators**: removed  `Multicast`
- **observable**: removed `ObservableOnce`
- **observable**: `observable::from_vec` and `observable::from_range` functions merge to `observable::from_iter!` macro.
- **observable**: `observable::empty` function  to `observable::empty!` macro.
- **observable**: `observable::of` function to `observable::of!` macro.
- **observable**: `observable::from_future` function to `observable::from_future!` macro
- **observable**: `observable::from_future_with_err` function to `observable::from_future_with_err!` macro
- **observable**: `observable::interval` function to `observable::interval!` macro

### Bug Fixes

- **observe_on**: unsubscribe should also cancel dispatched message.
- **subscribe_on**: unsubscribe should also cancel task in scheduler queue.

## [0.2.0](https://github.com/rxRust/rxRust/releases/tag/v0.2.0)  (2019-09-02)

### Features
- **observable**: add `observable::from_vec` and `observable::from_range`
- **observable**: add `observable::empty` and `observable::of`
- **observable**: add `observable::from_future` and `observable::from_future_with_err`
- **observable**: add `observable::interval`
- **operator**: add `delay` operator 
- **operator**: add `filter` operator 
- **operator**: add `first` operator 
- **operator**: add `multicast` and `fork` operator, `multicast` and `fork` are  special operators in rxrust, that because in rxrust all operators both consume the upstream, so the are unicast, `multicast` let you can convert an unicast stream to a multicast stream to support `fork` stream from it.
- **operator**: add `map` operator 
- **operator**: add `merge` operator
- **operator**: add `observe_on` operator
- **operator**: add `subscribe_on` operator
- **operator**: add `take` operator
- **Schedulers**: add `Schedulers::Sync` implementation
- **Schedulers**: add `Schedulers::NewThread` implementation
- **Schedulers**: add `Schedulers::ThreadPool` implementation
