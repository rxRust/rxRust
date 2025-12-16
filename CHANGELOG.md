# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

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

### 🛠️ Advanced Capabilities

*   **Custom Schedulers**: Inject custom schedulers (e.g., for Game Loops, GUI Event Queues, or Test Virtual Time) using the new Type Alias pattern (e.g., `type GameRx = LocalCtx<T, GameScheduler>`).
*   **Environment-Agnostic Operators**: A new guide and traits (`CoreObservable`, `ObservableType`) for authoring custom operators that work across all contexts without code duplication.

### 💔 Sorry & Breaking Changes

We sincerely apologize for the long delay in reaching version 1.0 and for the significant breaking changes introduced in this release. The core trait structure has undergone a complete overhaul. This difficult decision was made to address the inherent complexity of Rust's generics and to finally establish a stable, unified API foundation. We realized that without these fundamental changes, the library could not evolve sustainably.

*   **API Unification**: Explicit types like `LocalObservable` and `SharedObservable` from previous beta versions are replaced by the `Local::of(...)` and `Shared::of(...)` factory patterns.
*   **Scheduler Usage**: Explicit scheduler arguments have been removed from standard operators in favor of context-bound defaults. Use `_with` variants (e.g., `delay_with`) for manual control.

<!-- next-url -->