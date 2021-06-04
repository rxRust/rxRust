## [Unreleased](https://github.com/rxRust/rxRust/compare/v0.13.0...HEAD)

## [0.13.0](https://github.com/rxRust/rxRust/compare/v0.12.0...HEAD)
### Features
- **tooling**: Make runnable on rust stable by
1. Removing declarative macros
2. Using bencher lib instead of nightly `test::Bencher`
3. Remove use of drain_filter
4. Remove InnerDeref using GAT

### Breaking Changes
- **Subject** remove factory method `Subject::new` and replace with `LocalSubject::new` as well as `SharedSubject::new`

## [0.12.0](https://github.com/rxRust/rxRust/compare/v0.11.0...HEAD)
### Features
- **operator**: add `flatten` operator.

### Breaking Changes
- **SharedObservable**:  rename `SharedObservable::to_shared` as `SharedObservable::into_shared`

## [0.11.0](https://github.com/rxRust/rxRust/compare/v0.10.0...HEAD)
### Features
- **operator**: add `element_at` operator.
- **operator**: add `ignore_elements` operator.
- **operator**: add `all` operator.
- **operator**: add `contains` operator.

### Refactor
- **operator**: `skip_last` should emit not only when observer complete.
- **operator**: make `first_or` implement with `first` and `default_if_empty`
- **operator**: make `last_or` implement with `last` and `default_if_empty`
- **operator**: make `ignore_elements` implement with `filter`

### Bug Fixes 

- make `interval` mod public.

## [0.10.0](https://github.com/rxRust/rxRust/compare/v0.10.0...HEAD)
### Features
- **operator**: add `distinct` operator.
- **operator**: add `debounce` operator.
- **subject**: export `LocalSubjectRef`, `LocalSubjectErrRef` and `LocalSubjectRefAll`.

### Breaking Changes

- `Observer` trait now use associated type replace generic type.


## [0.9.1](https://github.com/rxRust/rxRust/releases/tag/v0.9.1)  (2020-08-25)

### Features
- **operator**: export `filter_map` in `Observable`.

## [0.9.0](https://github.com/rxRust/rxRust/releases/tag/v0.9.0)  (2020-08-22)

### Features
- **operator**: add `map_to` operator.
- **operator**: add `finalize` operator.
- **subscription**: Add `SubscriptionGuard::new()` for enabling RAII for existing subscriptions.
- **subscription**: Add `SubscriptionWrapper::into_inner()`, e.g. if one wants to add the inner subscription to
  a composite subscription.
- **scheduler**: Add two trait `SharedScheduler` and `LocalScheduler` to implement custom Scheduler.
- **scheduler**: `LocalPool` and `ThreadPool` in `futures::executor` can directly use as scheduler.
- **scheduler**: `tokio::runtime::Runtime` also supported, but need enable the feature `futures-scheduler`.
- **observer**: add a `is_stopped` method for `Observer` to detect if emit completed.

### Refactor

- **scheduler**: Use the runtime of future as the scheduler, and the default scheduler has be removed.

### Bug Fixes

- **operator**: `observer_on` not really emit value from immediate observable like `observable::of`.

### Breaking Changes

- **scheduler**: `Schedulers` has been removed.
- **observable**: don't require items/errors to implement `PayloadCopy`, `Clone` is enough now (remove `PayloadCopy`)
- **observable**: `observable::from_future` and `observable::interval` need give `scheduler` parameter.
- **operator**: `delay`,`observer_on` and `subscribe_on` need give `scheduler` parameter.
- **subscription**: remove method `inner_addr` in `SubscriptionLike`.
- **subject** remove `MutRefSubject`.

## [0.8.3](https://github.com/rxRust/rxRust/releases/tag/v0.8.2)  (2020-03-26)

### Bug Fixes

- **operator**: `sample` support clone, and not require source observer and sample observable return same subscription.

## [0.8.2](https://github.com/rxRust/rxRust/releases/tag/v0.8.2)  (2020-03-25)

### Breaking Changes

**operator**: add some explicit bounds on operators method to improve type infer, and some code use `map` may not compile, if it's just `map` and never subscribe.
**Subject**: MutRefSubject now mark as unsafe.

### Bug Fixes

- **operator**: remove unnecessary lifetime bounds on `box_it` operator.

### Features
- **subscription** The guard returned by `unsubscribe_when_dropped()` has the [must_use](https://doc.rust-lang.org/reference/attributes/diagnostics.html#the-must_use-attribute) attribute
- **operator**: add `zip` operator.
- **operator**: add `take_until` operator.
- **operator**: add `take_while` operator.
- **operator**: add `share` operator.
- **operator**: add `default_if_empty` operator.
- **observer**: add support for items/errors that don't implement `Copy` (by implementing `PayloadCopy`)
- **observable**: add macros `of_sequence` that producing values from a custom sequence.
- **subject**: add `subscribed_size` method on Subject.

## [0.8.1](https://github.com/rxRust/rxRust/releases/tag/v0.8.1)  (2020-02-28)

- **docs**: fix docs link and remove inner macro from docs.

## [0.8.0](https://github.com/rxRust/rxRust/releases/tag/v0.8.0)  (2020-02-28)

### Features

- **operator**: add `box_it` operator to box observable.
- **operator**: add `skip` operator.
- **operator**: add `skip_last` operator.
- **operator**: add `take_last` operator.
- **subscription** The return value of `subscribe`, `subscribe_err`, `subscribe_complete` and `subscribe_all` now
  provides a method `unsubscribe_when_dropped()` which activates "RAII" behavior for this subscription. That means
  `unsubscribe()` will be called automatically as soon as the value returned by `unsubscribe_when_dropped()` goes out
  of scope. If you don't assign the return value to a variable, `unsubscribe()` is called immediately!

### Refactor

- **observable**: Operators as provided methods on Observable instead of extension traits.
- **observable**: Every observable creation function has a concrete type, not only use a LocalObservable struct to wrap all,
- **observable** rename `RawSubscribable` to `LocalObservable`

### Breaking Changes

- **operator**: all operator extension traits are removed.
- **observable**: remove `Observable::new`, and add a same `create` function in `observable` to replace it.
- **observable**: Rename `Observable` to `ObservableFromFn`.
- **operator**: Remove `IntoShared` trait.
- **operator**: Use `Clone` replace `Fork`, now just call `observable.clone()` replace `observable.fork`.
- **subject**: merge `Subject::local` and `Subject::new` into `Subject::new`.
- **subject**: For now, LocalSubject emit value by mut ref must explicit call `mut_ref_all`, `mut_ref_item` and `mut_ref_err`. For example:
    ```rust
        let subject = Subject::new().mut_ref_item().subscribe(|_|{});
        subject.next(&mut 1);
    ```
- **observable**: rename observable creation function `from_fn` to `of_fn`
- **observable**: rename observable creation function `from_future_with_err` to `from_future_result`
- **observable**: Redefine `RawSubscribable` as `LocalObservable`. From
  ```rust
  pub trait RawSubscribable<Subscriber> {
    type Unsub: SubscriptionLike + 'static;
    fn raw_subscribe(self, subscriber: Subscriber) -> Self::Unsub;
  }
  ```
  to

  ```rust
  pub trait LocalObservable<'a> {
    type Item;
    type Err;
    type Unsub: SubscriptionLike + 'static;
    fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'a>
    (
      self,
      subscriber: Subscriber<O, LocalSubscription>,
    ) -> Self::Unsub;
  }
  ```

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
  `from_future_with_err!` replaced by functions.

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
let res = Arc::new(Mutex(0));
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
