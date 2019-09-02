## [Unreleased](https://github.com/M-Adoo/rxRust/compare/v0.2.0...HEAD)
todo

## [0.2.0](https://github.com/M-Adoo/rxRust/releases/tag/v0.2.0)  (2019-09-02)

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
- **operator**: add `take_on` operator
- **Schedulers**: add `Schedulers::Sync` implementation
- **Schedulers**: add `Schedulers::NewThread` implementation
- **Schedulers**: add `Schedulers::ThreadPool` implementation
