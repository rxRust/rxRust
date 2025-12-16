# Advanced Topics

Deep dives into extending rxRust and understanding its architecture.

rxRust v1.0 is built on a **Unified Architecture** where the `Context` drives everything. This design allows operators to be implemented once ("Environment-Agnostic") and automatically adapt to single-threaded (`Local`) or multi-threaded (`Shared`) environments.

This section is for library authors, framework integrators, and users who need to customize rxRust's execution model.

## Core Concepts for Extensions

To effectively extend rxRust, it helps to understand the three pillars of its architecture:

1.  **Context (`Context` Trait)**:
    The container that defines the *execution environment*. It carries the **Scheduler** (e.g., `LocalScheduler`, `TokioScheduler`) and the **Inner Logic** (the operator chain). It handles the "how" and "where" of execution.

2.  **Logic Kernel (`CoreObservable` Trait)**:
    The pure logic of an operator (e.g., `Map`, `Filter`). It is generic over the `Context`, allowing it to be "injected" into any environment.

3.  **Type Propagation (`ObservableType` Trait)**:
    A dedicated trait that exposes the `Item` and `Err` types, ensuring that type inference works cleanly across the complex generic boundaries of the Context.

## In This Section

### [Architecture Deep Dive](advanced/architecture_deep_dive.md)
Understand the foundational traits (`Context`, `CoreObservable`, `ObservableType`) that make rxRust's unified architecture possible.
*   **Context as Environment**: Explore how the `Context` trait carries the scheduler and manages ownership strategy.
*   **Type Propagation**: Learn about `Self::With<T>` and how it ensures type safety and environment-agnostic operator chaining.

### [Custom Operators](advanced/custom_operators.md)
Learn how to implement first-class operators that feel native to rxRust.
*   **Write Once, Run Everywhere**: Create operators that work in both `Local` and `Shared` contexts without code duplication.
*   **The `CoreObservable` Pattern**: See how to implement the glue logic that connects your operator to the Context.
*   **Performance**: How rxRust's zero-cost abstractions ensure your custom operators compile down to optimized code.

### [Nightly (Experimental)](advanced/nightly.md)
Notes on nightly-only experiments (currently: lifetime-dependent mapped outputs in `map`).

### [Custom Schedulers & Runtimes](advanced/custom_scheduling.md)
Learn how to take control of time and execution.
*   **Game Loops**: Synchronize `interval`, `timer`, and `delay` operators with your game's tick loop.
*   **GUI Integration**: Integrate rxRust with GTK, Qt, or web-based event loops.
*   **Virtual Time Testing**: Use deterministic schedulers to test time-dependent logic reliably.
*   **The Type Alias Pattern**: How to inject your custom scheduler into `Local` or `Shared` contexts with zero runtime overhead.