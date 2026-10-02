# Changelog

All notable changes to this project will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [Unreleased] - ReleaseDate

### 🎉 1.0.0 Release: The Unified Architecture

Welcome to rxRust v1.0! This release represents a complete reimplementation of the library, moving to a **Context-Driven Architecture**. This design solves the "Thread-Safety vs. Performance" dilemma by allowing the same operator logic to adapt automatically to single-threaded (`Local`) or multi-threaded (`Shared`) environments at compile time.

### 🚀 Major Architectural Changes

*   **Unified Context System**: Introduced the `Context` trait as the foundation of all streams.
    *   **`Local` Context**: Optimized for single-threaded environments (WASM, UI Main Threads). Uses `Rc<RefCell<T>>` internally for zero-locking overhead.
    *   **`Shared` Context**: Thread-safe implementation for concurrency. Uses `Arc<Mutex<T>>` and requires `Send + Sync`.
*   **Implicit Scheduling**: Schedulers are now bound to the Context.
    *   `Local` streams automatically use `LocalScheduler` (current thread/microtasks).
    *   `Shared` streams automatically use `SharedScheduler` (thread pool/tokio).
    *   Time-based operators (`delay`, `debounce`, `throttle`) no longer require explicit scheduler arguments by default.
*   **Zero-Cost Abstractions**: Heavy reliance on generic specialization (`CoreObservable`) ensures that operator chains compile down to efficient, monomorphized code with no unnecessary runtime overhead.

### ✨ New Features

*   **Async Interoperability**:
    *   `from_future` / `into_future`: Convert between Rust Futures and Observables.
    *   `from_stream` / `into_stream`: Seamlessly bridge Rust `Stream` (Tokio/Async-std) with Rx operators.
*   **Enhanced Operator Suite**: A comprehensive set of operators including:
    *   **Transformation**: `switch_map`, `concat_map`, `flat_map`, `scan`, `reduce`, `buffer`, `window`.
    *   **Filtering**: `debounce`, `throttle`, `sample`, `distinct_until_changed`.
    *   **Combination**: `combine_latest`, `with_latest_from`, `zip`, `merge`, `concat`.
    *   **Utility**: `retry`, `tap`, `delay`, `observe_on`, `subscribe_on`.
*   **WASM Support**: First-class support for WebAssembly via `Local` context, enabling high-performance reactive web apps.
*   **Subject Improvements**: `Subject` and `BehaviorSubject` now support "Multicasting" and adapt their internal locking strategy based on the Context they are created in.
*   **RxJS Parity, `group_by` options**: `group_by_with_duration` and `group_by_connector` (RxJS `groupBy` `duration` / `connector`).
*   **RxJS Parity, `share` config**: `share_with`, `share_replay_with`, `share_connector` with `ShareConfig` (RxJS 7 `resetOnError` / `resetOnComplete` / `resetOnRefCountZero`), `Connector` trait with `PublishConnector` and `ReplayConnector`.
*   **RxJS Parity, Tier 3**: `switch_scan`, `window_toggle`, `window_when`, `debounce_when` (duration selector), `sample_time`, `replay_subject_with_window` (time-windowed replay), plus RxJS-named entry points `merge_map`, `merge_with`, `zip_with`, `race_with`, `combine_latest_with`, `switch_all`, `exhaust_all`, `to_vec`, and `#[doc(alias)]` RxJS names on the new items.
*   **RxJS Parity, Tier 2b**: `window`, `window_count`, `window_time`, `buffer_when`, `buffer_toggle`, `delay_when`, `merge_scan`, `expand`.
*   **RxJS Parity, Tier 2a**: `partition`, `sequence_equal`, `single` with `SingleError`, `on_error_resume_next`, and the factories `generate`, `iif`, `from_callback`, `using`.
*   **RxJS Parity, Tier 1b**: `ReplaySubject`, `AsyncSubject`, `share`, `share_replay`, `publish_replay`, `publish_behavior`, `publish_last`, `catch_error`, the `timeout` family with `TimeoutError`, `repeat`, `repeat_forever`, `exhaust_map`, `audit`, `audit_time`.
*   **RxJS Parity, Tier 1a**: `every`, `ignore_elements`, `is_empty`, `element_at`, `element_at_or`, `find`, `find_index`, `end_with`, `throw_if_empty`, `materialize`, `dematerialize`, `timestamp`, `time_interval`, `race`, and the N-ary factories `race_observables`, `fork_join_observables`, `combine_latest_observables`, `zip_observables`.

### 🛠️ Advanced Capabilities

*   **Custom Schedulers**: Inject custom schedulers (e.g., for Game Loops, GUI Event Queues, or Test Virtual Time) using the new Type Alias pattern (e.g., `type GameRx = LocalCtx<T, GameScheduler>`).
*   **Environment-Agnostic Operators**: A new guide and traits (`CoreObservable`, `ObservableType`) for authoring custom operators that work across all contexts without code duplication.

### 💔 Sorry & Breaking Changes

We sincerely apologize for the long delay in reaching version 1.0 and for the significant breaking changes introduced in this release. The core trait structure has undergone a complete overhaul. This difficult decision was made to address the inherent complexity of Rust's generics and to finally establish a stable, unified API foundation. We realized that without these fundamental changes, the library could not evolve sustainably.

*   **API Unification**: Explicit types like `LocalObservable` and `SharedObservable` from previous beta versions are replaced by the `Local::of(...)` and `Shared::of(...)` factory patterns.
*   **Scheduler Usage**: Explicit scheduler arguments have been removed from standard operators in favor of context-bound defaults. Use `_with` variants (e.g., `delay_with`) for manual control.
*   **BehaviorSubject**: the current value now lives behind the context's shared pointer, so every clone observes the latest value. The type is `BehaviorSubject<ValuePtr, P>` instead of `BehaviorSubject<Item, P>`.
*   **Multicasting**: `ConnectableObservable` and `RefCount` are generic over the subject type via the new `MulticastSubject` trait, and `multicast` accepts any subject.
