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
- [x] From — convert some other object or data structure into an Observable
  - [x] `from_iter`
  - [x] `from_fn`
  - [x] `from_future`
  - [x] `from_stream`
  - [x] `from_callback`
  - [x] `generate` (state-machine iterator)
- [x] Interval — create an Observable that emits a sequence of integers spaced by a particular time interval
- [x] Just — convert an object or a set of objects into an Observable that emits that or those objects
  - named `of`, `of_result`, `of_option` in rxRust
- [x] Range — create an Observable that emits a range of sequential integers
  - use `from_iter` with a range
- [x] Repeat — create an Observable that emits a particular item or sequence of items repeatedly
  - implemented as `repeat(count)` / `repeat_forever()`, resubscribing on the scheduler's next tick
- [x] Start — create an Observable that emits the return value of a function
  - `defer` or `from_fn` covers this.
- [x] Timer — create an Observable that emits a single item after a given delay
- [x] Iif — choose between two Observables at subscribe time (`iif`)

### Transforming Observables

Operators that transform items that are emitted by an Observable.

- [x] Buffer — periodically gather items from an Observable into bundles and emit these bundles rather than emitting the items one at a time
  - [x] `buffer(closing_notifier)`
  - [x] `buffer_with_count`
  - [x] `buffer_with_time`
  - [x] `buffer_with_count_and_time`
  - [x] `buffer_when(closing_selector)`
  - [x] `buffer_toggle(openings, closing_selector)`
- [x] FlatMap — transform the items emitted by an Observable into Observables, then flatten the emissions from those into a single Observable
  - implemented as `merge_all` (flatten) or `map(...).merge_all(...)`
  - [x] ExhaustMap — ignore outer items while an inner Observable is active (`exhaust_map`)
- [x] GroupBy — divide an Observable into a set of Observables that each emit a different group of items from the original Observable, organized by key
  - `group_by(key)`, `group_by_with_duration(key, duration)`, `group_by_connector(key, connector, duration)` (RxJS `duration` / `connector` options)
- [x] Partition — split an Observable into the items matching a predicate and the rest (`partition`)
- [x] Map — transform the items emitted by an Observable by applying a function to each item
- [x] Scan — apply a function to each item emitted by an Observable, sequentially, and emit each successive value
- [x] Materialize/Dematerialize — represent both the items emitted and the notifications sent as emitted items, or reverse this process
  - `materialize` emits `Notification<Item, Err>`; `dematerialize` replays them
- [x] Window — periodically subdivide items from an Observable into Observable windows and emit these windows rather than emitting the items one at a time
  - `window(notifier)`, `window_count(count)`, `window_time(duration)`, `window_when(closing_selector)`, `window_toggle(openings, closing_selector)`
- [x] MergeScan — accumulate through observables (`merge_scan`)
- [x] SwitchScan — accumulate through observables, switching to the latest (`switch_scan`)
- [x] Expand — recursively project and merge (`expand`)

### Filtering Observables

Operators that selectively emit items from a source Observable.

- [x] Debounce — only emit an item from an Observable if a particular timespan has passed without it emitting another item
  - [x] Throttle
  - [x] ThrottleTime
  - [x] Debounce (`debounce(duration)`; `debounce_when(selector)` for a per-item duration Observable)
  - [x] Audit / AuditTime (`audit`, `audit_time`, `audit_time_with`)
- [x] Distinct — suppress duplicate items emitted by an Observable
  - [x] DistinctUntilChanged — only emit when the current value is different than the last
- [x] ElementAt — emit only item n emitted by an Observable
  - implemented as `element_at` / `element_at_or`
- [x] Filter — emit only those items from an Observable that pass a predicate test
- [x] First — emit only the first item, or the first item that meets a condition, from an Observable
  - `first`, `first_or`, `find`, `find_index`
- [x] IgnoreElements — do not emit any items from an Observable but mirror its termination notification
  - implemented as `ignore_elements`
- [x] Last — emit only the last item emitted by an Observable
- [x] Single — emit the only item, or error with `SingleError` (`single`)
- [x] Sample — emit the most recent item emitted by an Observable within periodic time intervals
  - `sample(notifier)`, `sample_time(period)`
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
  - N-ary: `combine_latest_observables`
- [x] ForkJoin — wait for all Observables to complete, then emit their last values (`fork_join_observables`)
- [ ] Join — combine items emitted by two Observables whenever an item from one Observable is emitted during a time window defined according to an item emitted by the other Observable
- [x] Merge — combine multiple Observables into one by merging their emissions
- [x] Race/Amb — mirror the first of several Observables to emit
  - `race` (binary), `race_observables` (N-ary)
- [x] StartWith — emit a specified sequence of items before beginning to emit the items from the source Observable
- [x] EndWith — emit a specified sequence of items after the source Observable completes (`end_with`)
- [x] Switch — convert an Observable that emits Observables into a single Observable that emits the items emitted by the most-recently-emitted of those Observables
  - available via `switch_map(|x| x)` (aka switchAll)
- [x] SwitchMap — map each item into an inner Observable and switch to the latest one
- [x] WithLatestFrom - similar to CombineLatest, but only emits items when the single source Observable emits an item
- [x] Zip — combine the emissions of multiple Observables together via a specified function and emit single items for each combination based on the results of this function
  - N-ary: `zip_observables`

### Error Handling Operators

Operators that help to recover from error notifications from an Observable

- [x] Catch — recover from an onError notification by continuing the sequence without error
  - implemented as `catch_error`; `map_err` transforms the error type
- [x] OnErrorResumeNext — continue with another Observable on error or completion (`on_error_resume_next`)
- [x] Retry — if a source Observable sends an onError notification, resubscribe to it in the hopes that it will complete without error
  - Implemented with generic policies (`count`, `delay`, `reset_on_success`).

### Observable Utility Operators

A toolbox of useful Operators for working with Observables

- [x] Delay — shift the emissions from an Observable forward in time by a particular amount
  - `delay`, `delay_at`, `delay_subscription`, and per-item `delay_when(selector)`
- [x] Do — register an action to take upon a variety of Observable lifecycle events
  - named `tap`
- [x] ObserveOn — specify the scheduler on which an observer will observe this Observable
- [ ] Serialize — force an Observable to make serialized calls and to be well-behaved
- [x] Subscribe — operate upon the emissions and notifications from an Observable
- [x] SubscribeOn — specify the scheduler an Observable should use when it is subscribed to
- [x] TimeInterval — convert an Observable that emits items into one that emits indications of the amount of time elapsed between those emissions
  - implemented as `time_interval`, emits `Elapsed { value, interval }`
- [x] Timeout — mirror the source Observable, but issue an error notification if a particular period of time elapses without any emitted items
  - `timeout`, `timeout_with`, `timeout_or_else`, `timeout_or_else_with`; emits `TimeoutError` by default
- [x] Timestamp — attach a timestamp to each item emitted by an Observable
  - implemented as `timestamp`, emits `Timestamped { value, timestamp }`
- [x] Using — create a disposable resource that has the same lifespan as the Observable
  - implemented as `using(resource_factory, observable_factory)`

### Conditional and Boolean Operators

Operators that evaluate one or more Observables or items emitted by Observables

- [x] All — determine whether all items emitted by an Observable meet some criteria
  - implemented as `every` (doc alias `all`)
- [x] Amb — given two or more source Observables, emit all of the items from only the first of these Observables to emit an item
  - see Race above
- [x] Contains — determine whether an Observable emits a particular item or not
- [x] DefaultIfEmpty — emit items from the source Observable, or a default item if the source Observable emits nothing
- [x] ThrowIfEmpty — error instead of completing when the source Observable emits nothing (`throw_if_empty`)
- [x] IsEmpty — emit whether the source Observable completed without items (`is_empty`)
- [x] SequenceEqual — determine whether two Observables emit the same sequence of items (`sequence_equal`)
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
- [x] Share — `publish().ref_count()` shortcut (`share`)
  - `share_with(config)` / `share_replay_with(capacity, config)` / `share_connector(connector, config)`: RxJS 7 `share` with `resetOnError` / `resetOnComplete` / `resetOnRefCountZero`
- [x] PublishBehavior / PublishLast — multicast through a `BehaviorSubject` or `AsyncSubject` (`publish_behavior`, `publish_last`)
- [x] Replay — ensure that all observers see the same sequence of emitted items, even if they subscribe after the Observable has begun emitting items
  - `publish_replay(capacity)`, `share_replay(capacity)`

### Operators to Convert Observables

- [x] Future - `to_future` converts an observable to a `Future`
- [x] Stream - `to_stream` converts an observable to `Stream`
- [x] To — convert an Observable into another object or data structure
  - `collect`, `collect_into`, `to_vec` (RxJS `toArray`)

## Subjects

- [x] AsyncSubject — emits the last value (and only the last value) emitted by the source Observable, and only after that source Observable completes
  - `Local::async_subject()` / `Shared::async_subject()`
- [x] BehaviorSubject — begins by emitting the item most recently emitted by the source Observable (or a seed/default value if none has yet been emitted) and then continues to emit any other items emitted later by the source Observable(s)
- [x] PublishSubject — emits to an observer only those items that are emitted by the source Observable(s) subsequent to the time of the subscription
  - The standard `Subject` in rxRust (`Local::subject()` / `Shared::subject()`) behaves as a PublishSubject.
- [x] ReplaySubject — emits to any observer all of the items that were emitted by the source Observable(s), regardless of when the observer subscribes
  - `replay_subject(capacity)` / `replay_subject_unbounded()` / `replay_subject_with_window(capacity, window)`

## Schedulers

- [x] Async Scheduler and timer for local thread (not thread safe).
- [x] Local thread scheduler.
- [x] Thread pool scheduler.
- [x] Virtual timer (for testing).

## Workflows

- [ ] CI
  - [x] Unit test coverage report.
  - [x] Benchmark to measure performance for every commit (`cargo bench`, tracked by github-action-benchmark on the `benchmarks` branch).
  - [ ] Real-life representative algorithms implemented to measure performance.