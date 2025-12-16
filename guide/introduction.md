# Introduction

Welcome to **rxRust** v1.0!

rxRust is a zero-cost, ergonomic, and functional reactive programming (FRP) library for Rust. It brings the widely adopted [ReactiveX](http://reactivex.io) pattern to the Rust ecosystem, designed specifically for scenarios where standard Async Streams fall short. It serves as the core reactive engine for high-performance GUI frameworks like [Ribir](https://github.com/RibirX/Ribir).

## Push vs. Pull: Why Observable?

In Rust, we are familiar with `Iterator` and `Future`/`Stream`. These are **Pull-based** systems: the consumer decides when to ask for the next value ("Give me the next item"). This works perfectly for reading files or database cursors.

However, real-world applications—especially GUIs, Web Apps, and IoT—are **Push-based**.
- A user clicks a button.
- A WebSocket message arrives.
- A timer fires.

You cannot "ask" a mouse for the next click; the mouse "pushes" the click to you.

rxRust's **Observable** is the standard for **Push-based** composition. It allows you to orchestrate complex event streams declaratively, rather than managing messy callbacks or state machines.

> "ReactiveX is a combination of the best ideas from the Observer pattern, the Iterator pattern, and functional programming." — [ReactiveX.io](http://reactivex.io)

## The "Aha!" Moment

Why choose rxRust over raw `tokio` channels or streams?

Consider a "Type-ahead Search" (Auto-complete):
1.  Wait for 300ms silence (**Debounce**).
2.  Ignore if the text hasn't changed (**Distinct**).
3.  Cancel the previous request if a new one starts (**Switch**).

With rxRust, complex timing and state logic become a linear pipeline:

```rust,ignore
use rxrust::prelude::*;

input_events
    .debounce(Duration::from_millis(300))
    .distinct_until_changed()
    .switch_map(|text| api_search(text)) // Automatically cancels old requests
    .subscribe(|results| render(results));
```

## Why rxRust Specifically?

While there are other reactive libraries, rxRust is built with a unique philosophy:

### 1. WASM & GUI First (Local Context)
Most Rust async libraries force `Send + Sync` constraints to support multi-threaded runtimes. This incurs unnecessary locking overhead (`Arc<Mutex<T>>`) for single-threaded environments like **WebAssembly (WASM)** or **UI Main Threads**.

rxRust provides a `Local` context based on `Rc<RefCell>`, offering **zero-locking overhead** for these high-performance client-side scenarios.

### 2. Unified Concurrency (Shared Context)
Need to scale to a multi-threaded server? Just switch to the `Shared` context. The API remains the same, but the implementation automatically switches to thread-safe primitives (`Arc<Mutex>`) and work-stealing schedulers.

### 3. Seamless Interop
We believe Push and Pull should coexist. rxRust makes it trivial to bridge the worlds:
- **`from_stream`**: Turn a Tokio Stream into an Observable to apply complex time-based operators.
- **`into_stream`**: Turn an Observable back into a Stream to consume it in a standard `async` loop.