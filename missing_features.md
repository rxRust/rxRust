# Missing features

## Operators By Category

According to http://reactivex.io/documentation/operators.html

### Creating Observables

Operators that originate new Observables.

- [x] Create — create an Observable from scratch by calling observer methods programmatically
  - use `new` method in rxRust
- [ ] Defer — do not create the Observable until the observer subscribes, and create a fresh Observable for each observer
- [x] Empty/Never/Throw — create Observables that have very precise and limited behavior
- [ ] From — convert some other object or data structure into an Observable
  - [x] `from_iter`
  - [x] `from_fn`
  - [ ] `from_callback`
- [x] Interval — create an Observable that emits a sequence of integers spaced by a particular time interval
- [x] Just — convert an object or a set of objects into an Observable that emits that or those objects
  - named `of`, `of_result`, `of_option` in rxRust, maybe renamed by `just` ?
- [x] Range — create an Observable that emits a range of sequential integers
- [x] Repeat — create an Observable that emits a particular item or sequence of items repeatedly
- [ ] Start — create an Observable that emits the return value of a function
- [ ] Timer — create an Observable that emits a single item after a given delay

### Transforming Observables

Operators that transform items that are emitted by an Observable.

- [ ] Buffer — periodically gather items from an Observable into bundles and emit these bundles rather than emitting the items one at a time
- [ ] FlatMap — transform the items emitted by an Observable into Observables, then flatten the emissions from those into a single Observable
- [ ] GroupBy — divide an Observable into a set of Observables that each emit a different group of items from the original Observable, organized by key
- [x] Map — transform the items emitted by an Observable by applying a function to each item
- [x] Scan — apply a function to each item emitted by an Observable, sequentially, and emit each successive value
- [ ] Window — periodically subdivide items from an Observable into Observable windows and emit these windows rather than emitting the items one at a time

### Filtering Observables

Operators that selectively emit items from a source Observable.

- [ ] Debounce — only emit an item from an Observable if a particular timespan has passed without it emitting another item
  - [x] ThrottleTime
  - [ ] Debounce
- [ ] Distinct — suppress duplicate items emitted by an Observable
- [ ] ElementAt — emit only item n emitted by an Observable
- [x] Filter — emit only those items from an Observable that pass a predicate test
- [x] First — emit only the first item, or the first item that meets a condition, from an Observable
- [ ] IgnoreElements — do not emit any items from an Observable but mirror its termination notification
- [x] Last — emit only the last item emitted by an Observable
- [ ] Sample — emit the most recent item emitted by an Observable within periodic time intervals
- [x] Skip — suppress the first n items emitted by an Observable
- [x] SkipLast — suppress the last n items emitted by an Observable
- [x] Take — emit only the first n items emitted by an Observable
- [x] TakeLast — emit only the last n items emitted by an Observable

### Combining Observables
Operators that work with multiple source Observables to create a single Observable

- [ ] And/Then/When — combine sets of items emitted by two or more Observables by means of Pattern and Plan intermediaries
- [ ] CombineLatest — when an item is emitted by either of two Observables, combine the latest item emitted by each Observable via a specified function and emit items based on the results of this function
- [ ] Join — combine items emitted by two Observables whenever an item from one Observable is emitted during a time window defined according to an item emitted by the other Observable
- [x] Merge — combine multiple Observables into one by merging their emissions
- [ ] StartWith — emit a specified sequence of items before beginning to emit the items from the source Observable
- [ ] Switch — convert an Observable that emits Observables into a single Observable that emits the items emitted by the most-recently-emitted of those Observables
- [x] Zip — combine the emissions of multiple Observables together via a specified function and emit single items for each combination based on the results of this function

### Error Handling Operators
Operators that help to recover from error notifications from an Observable

- [ ] Catch — recover from an onError notification by continuing the sequence without error
- [ ] Retry — if a source Observable sends an onError notification, resubscribe to it in the hopes that it will complete without error

### Observable Utility Operators
A toolbox of useful Operators for working with Observables

- [x] Delay — shift the emissions from an Observable forward in time by a particular amount
- [ ] Do — register an action to take upon a variety of Observable lifecycle events
- [ ] Materialize/Dematerialize — represent both the items emitted and the notifications sent as emitted items, or reverse this process
- [x] ObserveOn — specify the scheduler on which an observer will observe this Observable
- [ ] Serialize — force an Observable to make serialized calls and to be well-behaved
- [ ] Subscribe — operate upon the emissions and notifications from an Observable
- [x] SubscribeOn — specify the scheduler an Observable should use when it is subscribed to
- [ ] TimeInterval — convert an Observable that emits items into one that emits indications of the amount of time elapsed between those emissions
- [ ] Timeout — mirror the source Observable, but issue an error notification if a particular period of time elapses without any emitted items
- [ ] Timestamp — attach a timestamp to each item emitted by an Observable
- [ ] Using — create a disposable resource that has the same lifespan as the Observable

### Conditional and Boolean Operators
Operators that evaluate one or more Observables or items emitted by Observables

- [ ] All — determine whether all items emitted by an Observable meet some criteria
- [ ] Amb — given two or more source Observables, emit all of the items from only the first of these Observables to emit an item
- [ ] Contains — determine whether an Observable emits a particular item or not
- [ ] DefaultIfEmpty — emit items from the source Observable, or a default item if the source Observable emits nothing
- [ ] SequenceEqual — determine whether two Observables emit the same sequence of items
- [ ] SkipUntil — discard items emitted by an Observable until a second Observable emits an item
- [ ] SkipWhile — discard items emitted by an Observable until a specified condition becomes false
- [ ] TakeUntil — discard items emitted by an Observable after a second Observable emits an item or terminates
- [ ] TakeWhile — discard items emitted by an Observable after a specified condition becomes false

### Mathematical and Aggregate Operators
Operators that operate on the entire sequence of items emitted by an Observable

- [x] Average — calculates the average of numbers emitted by an Observable and emits this average
- [ ] Concat — emit the emissions from two or more Observables without interleaving them
- [x] Count — count the number of items emitted by the source Observable and emit only this value
- [x] Max — determine, and emit, the maximum-valued item emitted by an Observable
- [x] Min — determine, and emit, the minimum-valued item emitted by an Observable
- [x] Reduce — apply a function to each item emitted by an Observable, sequentially, and emit the final value
- [x] Sum — calculate the sum of numbers emitted by an Observable and emit this sum

### Backpressure Operators

- [ ] backpressure operators — strategies for coping with Observables that produce items more rapidly than their observers consume them

### Connectable Observable Operators
Specialty Observables that have more precisely-controlled subscription dynamics

-  [x] Connect — instruct a connectable Observable to begin emitting items to its subscribers
-  [x] Publish — convert an ordinary Observable into a connectable Observable
-  [x] RefCount — make a Connectable Observable behave like an ordinary Observable
-  [ ] Replay — ensure that all observers see the same sequence of emitted items, even if they subscribe after the Observable has begun emitting items

### Operators to Convert Observables
- [ ] To — convert an Observable into another object or data structure

## Schedulers

- [ ] maybe Redesign? How should rxRust work with Future's executor more naturally.
- [ ] async Scheduler and timer for local thread, and not require thread safe.
- [x] new thread scheduler
- [x] thread pool scheduler

## Docs

- [ ] A guide to introduce how to implement an operator or Observable creator.

## Workflows

- [x] finish **Missing Features List** so that people can contribute easily
- [ ] issues and pr's template
- [ ] test, some discuss in https://github.com/orgs/rxRust/teams/core-developers/discussions/2
  - [ ] virtual timer
  - [ ] taking mature work from others Rx projects.
  - [ ] having some real life representative algorithms implemented to measure over performance,
  - [ ] more complex test, not just `bool` or `i32`. Maybe bringing a mocking library?
- [ ] ci
  - [x] unit test coverage report.
  - [ ] benchmark to measure the performance for every commit.