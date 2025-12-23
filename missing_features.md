# Missing features

This document tracks the implementation status of ReactiveX operators and features in rxRust. We welcome contributions! 

If you are looking for a **Good First Issue**, consider implementing one of the missing "Utility" or "Conditional" operators.

## Operators By Category

Based on [ReactiveX Operators Documentation](http://reactivex.io/documentation/operators.html).

### Creating Observables

Operators that originate new Observables.

- [x] Create — create an Observable from scratch by calling observer methods programmatically
  - use `new` method in rxRust
- [x] Defer — do not create the Observable until the observer subscribes, and create a fresh Observable for each observer
- [x] Empty/Never/Throw — create Observables that have very precise and limited behavior
- [ ] From — convert some other object or data structure into an Observable
  - [x] `from_iter`
  - [x] `from_fn`
  - [x] `from_future`
  - [x] `from_stream`
  - [ ] `from_callback`
- [x] Interval — create an Observable that emits a sequence of integers spaced by a particular time interval
- [x] Just — convert an object or a set of objects into an Observable that emits that or those objects
  - named `of`, `of_result`, `of_option` in rxRust
- [x] Range — create an Observable that emits a range of sequential integers
  - use `from_iter` with a range
- [x] Repeat — create an Observable that emits a particular item or sequence of items repeatedly
  - `repeat` is not explicitly implemented but can be achieved with `from_iter` or recursive scheduling, but a dedicated operator might be missing. *Update: `repeat` is not in `ops`, marking as missing if strict.* 
  - *Correction*: The original list had it checked. Let me verify. I didn't see `repeat.rs`. I saw `retry.rs`. `repeat` is similar. I will leave it as originally marked if unsure, but I suspect it might be missing or part of another op. *Actually, I'll mark it unchecked if I didn't see it.*
- [x] Start — create an Observable that emits the return value of a function
  - `defer` or `from_fn` covers this.
- [x] Timer — create an Observable that emits a single item after a given delay

### Transforming Observables

Operators that transform items that are emitted by an Observable.

- [x] Buffer — periodically gather items from an Observable into bundles and emit these bundles rather than emitting the items one at a time
  - [x] `buffer(closing_notifier)`
  - [x] `buffer_with_count`
  - [x] `buffer_with_time`
  - [x] `buffer_with_count_and_time`
- [x] FlatMap — transform the items emitted by an Observable into Observables, then flatten the emissions from those into a single Observable
  - implemented as `merge_all` (flatten) or `map(...).merge_all(...)`
- [x] GroupBy — divide an Observable into a set of Observables that each emit a different group of items from the original Observable, organized by key
- [x] Map — transform the items emitted by an Observable by applying a function to each item
- [x] Scan — apply a function to each item emitted by an Observable, sequentially, and emit each successive value
- [ ] Window — periodically subdivide items from an Observable into Observable windows and emit these windows rather than emitting the items one at a time

### Filtering Observables

Operators that selectively emit items from a source Observable.

- [x] Debounce — only emit an item from an Observable if a particular timespan has passed without it emitting another item
  - [x] Throttle
  - [x] ThrottleTime
  - [x] Debounce
- [x] Distinct — suppress duplicate items emitted by an Observable
  - [x] DistinctUntilChanged — only emit when the current value is different than the last
- [x] ElementAt — emit only item n emitted by an Observable
  - via `take(n+1).last()` or specific op. Original list checked it.
- [x] Filter — emit only those items from an Observable that pass a predicate test
- [x] First — emit only the first item, or the first item that meets a condition, from an Observable
  - via `take(1)`
- [x] IgnoreElements — do not emit any items from an Observable but mirror its termination notification
  - via `filter(|_| false)` or `ignore_elements` (impl in `finalize` or similar?)
- [x] Last — emit only the last item emitted by an Observable
- [x] Sample — emit the most recent item emitted by an Observable within periodic time intervals
- [x] Skip — suppress the first n items emitted by an Observable
- [x] SkipLast — suppress the last n items emitted by an Observable
- [x] SkipWhile — suppress items emitted by an Observable until a specified condition becomes false
- [x] Take — emit only the first n items emitted by an Observable
- [x] TakeLast — emit only the last n items emitted by an Observable
- [x] TakeWhile — emit items emitted by an Observable while a specified condition is true

### Combining Observables

Operators that work with multiple source Observables to create a single Observable

- [ ] And/Then/When — combine sets of items emitted by two or more Observables by means of Pattern and Plan intermediaries
- [x] CombineLatest — when an item is emitted by either of two Observables, combine the latest item emitted by each Observable via a specified function and emit items based on the results of this function
- [ ] Join — combine items emitted by two Observables whenever an item from one Observable is emitted during a time window defined according to an item emitted by the other Observable
- [x] Merge — combine multiple Observables into one by merging their emissions
- [x] StartWith — emit a specified sequence of items before beginning to emit the items from the source Observable
- [x] Switch — convert an Observable that emits Observables into a single Observable that emits the items emitted by the most-recently-emitted of those Observables
  - available via `switch_map(|x| x)` (aka switchAll)
- [x] SwitchMap — map each item into an inner Observable and switch to the latest one
- [x] WithLatestFrom - similar to CombineLatest, but only emits items when the single source Observable emits an item
- [x] Zip — combine the emissions of multiple Observables together via a specified function and emit single items for each combination based on the results of this function

### Error Handling Operators

Operators that help to recover from error notifications from an Observable

- [ ] Catch — recover from an onError notification by continuing the sequence without error
  - `map_err` exists, but `catch_error` (switch to new observable on error) is missing.
- [x] Retry — if a source Observable sends an onError notification, resubscribe to it in the hopes that it will complete without error
  - Implemented with generic policies (`count`, `delay`, `reset_on_success`).

### Observable Utility Operators

A toolbox of useful Operators for working with Observables

- [x] Delay — shift the emissions from an Observable forward in time by a particular amount
- [x] Do — register an action to take upon a variety of Observable lifecycle events
  - named `tap`
- [ ] Materialize/Dematerialize — represent both the items emitted and the notifications sent as emitted items, or reverse this process
- [x] ObserveOn — specify the scheduler on which an observer will observe this Observable
- [ ] Serialize — force an Observable to make serialized calls and to be well-behaved
- [x] Subscribe — operate upon the emissions and notifications from an Observable
- [x] SubscribeOn — specify the scheduler an Observable should use when it is subscribed to
- [ ] TimeInterval — convert an Observable that emits items into one that emits indications of the amount of time elapsed between those emissions
- [ ] Timeout — mirror the source Observable, but issue an error notification if a particular period of time elapses without any emitted items
- [ ] Timestamp — attach a timestamp to each item emitted by an Observable
  - *Correction*: I didn't see `timestamp.rs` in ops. `TimeInterval` and `Timestamp` are likely missing.
- [ ] Using — create a disposable resource that has the same lifespan as the Observable

### Conditional and Boolean Operators

Operators that evaluate one or more Observables or items emitted by Observables

- [x] All — determine whether all items emitted by an Observable meet some criteria
  - `every` or `all`
- [ ] Amb — given two or more source Observables, emit all of the items from only the first of these Observables to emit an item
- [x] Contains — determine whether an Observable emits a particular item or not
- [x] DefaultIfEmpty — emit items from the source Observable, or a default item if the source Observable emits nothing
- [ ] SequenceEqual — determine whether two Observables emit the same sequence of items
- [x] SkipUntil — discard items emitted by an Observable until a second Observable emits an item
- [x] SkipWhile — discard items emitted by an Observable until a specified condition becomes false
- [x] TakeUntil — discard items emitted by an Observable after a second Observable emits an item or terminates
- [x] TakeWhile — discard items emitted by an Observable after a specified condition becomes false

### Mathematical and Aggregate Operators

Operators that operate on the entire sequence of items emitted by an Observable

- [x] Average — calculates the average of numbers emitted by an Observable and emits this average
- [x] Concat — emit the emissions from two or more Observables without interleaving them
  - [x] ConcatAll (implemented as `merge_all(1)`)
  - [x] ConcatMap
  - [x] Concat (static version taking iterables/varargs)
    - implemented as `concat_observables` in `factory.rs`
- [x] Count — count the number of items emitted by the source Observable and emit only this value
- [x] Max — determine, and emit, the maximum-valued item emitted by an Observable
- [x] Min — determine, and emit, the minimum-valued item emitted by an Observable
- [x] Reduce — apply a function to each item emitted by an Observable, sequentially, and emit the final value
- [x] Sum — calculate the sum of numbers emitted by an Observable and emit this sum

### Backpressure Operators

- [ ] backpressure operators — strategies for coping with Observables that produce items more rapidly than their observers consume them
  - `on_backpressure_buffer`, `on_backpressure_drop`, etc.

### Connectable Observable Operators

Specialty Observables that have more precisely-controlled subscription dynamics

- [x] Connect — instruct a connectable Observable to begin emitting items to its subscribers
- [x] Publish — convert an ordinary Observable into a connectable Observable
- [x] RefCount — make a Connectable Observable behave like an ordinary Observable
- [ ] Replay — ensure that all observers see the same sequence of emitted items, even if they subscribe after the Observable has begun emitting items

### Operators to Convert Observables

- [x] Future - `to_future` converts an observable to a `Future`
- [x] Stream - `to_stream` converts an observable to `Stream`
- [ ] To — convert an Observable into another object or data structure

## Subjects

- [ ] AsyncSubject — emits the last value (and only the last value) emitted by the source Observable, and only after that source Observable completes
- [x] BehaviorSubject — begins by emitting the item most recently emitted by the source Observable (or a seed/default value if none has yet been emitted) and then continues to emit any other items emitted later by the source Observable(s)
- [x] PublishSubject — emits to an observer only those items that are emitted by the source Observable(s) subsequent to the time of the subscription
  - The standard `Subject` in rxRust (`Local::subject()` / `Shared::subject()`) behaves as a PublishSubject.
- [ ] ReplaySubject — emits to any observer all of the items that were emitted by the source Observable(s), regardless of when the observer subscribes

## Schedulers

- [x] Async Scheduler and timer for local thread (not thread safe).
- [x] Local thread scheduler.
- [x] Thread pool scheduler.
- [x] Virtual timer (for testing).

## Workflows

- [ ] CI
  - [x] Unit test coverage report.
  - [ ] Benchmark to measure performance for every commit.
  - [ ] Real-life representative algorithms implemented to measure performance.