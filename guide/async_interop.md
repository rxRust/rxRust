# Async Interoperability

rxRust is designed to play nicely with Rust's async ecosystem (Tokio, async-std, etc.). It fills the gap for complex event orchestration, allowing you to use ReactiveX patterns where standard `Future`s and `Stream`s might become cumbersome.

## 1. Future Interoperability: `from_future` / `into_future`

### `from_future`: Converting a `Future` to an `Observable`

You can easily turn any `Future` into an `Observable` using the `from_future` operator. This is useful when you have an asynchronous operation that produces a single value (or an error) and you want to integrate it into an Rx pipeline, leveraging operators like `timeout`, `retry`, or `switch_map`.

The `from_future` operator takes ownership of the `Future` and emits its `Output` value as `next` when the `Future` completes.

```rust
use rxrust::prelude::*;
use tokio::time::sleep;

async fn delayed_greeting(name: &str) -> String {
    sleep(Duration::from_secs(1)).await;
    format!("Hello, {}!", name)
}

#[tokio::main]
async fn main() {
    println!("Starting Future to Observable example...");

    // Create a Future
    let my_future = delayed_greeting("Alice");

    // Convert the Future into an Observable
    Shared::from_future(my_future)
        .subscribe(|msg| {
            println!("Observable received: {}", msg);
        });

    // The main function needs to complete for the async task to run.
    // In a real application, you might have a long-running executor.
    sleep(Duration::from_secs(2)).await;
    println!("Future to Observable example finished.");
}
```

### `into_future`: Converting an `Observable` to a `Future`

Conversely, if you have an `Observable` that you expect to emit a single value (e.g., from a network request that completes once, or after applying `first()` or `take(1)`), you can convert it into a `Future` using `into_future`.

This is particularly useful for integrating the final result of an Rx pipeline back into an `async/.await` control flow.

```rust
use rxrust::prelude::*;

#[tokio::main]
async fn main() {
    println!("Starting Observable to Future example...");

    // Create an Observable that emits a few values and then completes
    let observable = Shared::from_iter(vec![10, 20, 30])
        .filter(|&v| v > 25) // Only 30 passes
        .take(1); // Take only the first matching value

    // Convert the Observable into a Future
    let result_future = observable.into_future();

    // Await the result
    match result_future.await {
        Ok(Ok(value)) => println!("Future resolved with: {}", value),
        Ok(Err(err)) => println!("Future resolved with an observable error: {:?}", err),
        Err(IntoFutureError::Empty) => {
            println!("Future resolved with no value (observable was empty).")
        }
        Err(IntoFutureError::MultipleValues) => {
            println!("Future resolved with multiple values (expected exactly one).")
        }
    }

    // Another example: an observable that completes without emitting
    let empty_observable = Shared::from_iter(std::iter::empty::<i32>());
    let empty_result = empty_observable.into_future().await;
    assert!(matches!(empty_result, Err(IntoFutureError::Empty)));
}
```

## 2. Stream Interoperability: `from_stream` / `into_stream`

### `from_stream`: Converting a `Stream` to an `Observable`

Rust's standard `Stream` trait is a pull-based asynchronous iterator. `from_stream` allows you to take an existing `Stream` (e.g., from `tokio::io::split`, `tokio::sync::mpsc`, or `futures::stream`) and convert it into an `Observable`. This enables you to apply rxRust's powerful set of operators, especially those focused on time-based logic, error handling, or complex event patterns.

```rust,no_run
use rxrust::prelude::*;
use futures::stream::{self, StreamExt};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    println!("Starting Stream to Observable example...");

    // Create a simple async Stream
    let my_stream = stream::iter(vec![1, 2, 3, 4, 5])
        .then(|v| async move {
            sleep(Duration::from_millis(500)).await; // Simulate async delay
            v * 10
        });

    // Convert the Stream into an Observable
    Shared::from_stream(my_stream)
        .filter(|&v| v > 20) // RxRust operator
        .map(|v| format!("Item: {}", v))
        .subscribe(|msg| {
            println!("Observable from Stream received: {}", msg);
        });

    sleep(Duration::from_secs(3)).await;
    println!("Stream to Observable example finished.");
}
```

### `into_stream`: Converting an `Observable` to a `Stream`

For situations where you have an `Observable` pipeline and you want to consume its emissions using standard `async/.await` syntax or integrate it into a part of your application that expects a `Stream`, `into_stream` is your go-to.

The resulting `Stream` yields `Result<T, E>` items, and ends (`None`) when the `Observable` completes (or after emitting an error).

```rust,no_run
use rxrust::prelude::*;
use futures::StreamExt; // For .next() and .collect()

#[tokio::main]
async fn main() {
    println!("Starting Observable to Stream example...");

    // Create an Observable
    let observable = Shared::interval(Duration::from_millis(100))
        .take(3) // Emit 0, 1, 2 then complete
        .map(|v| v + 100);

    // Convert the Observable into a Stream
    let mut my_stream = observable.into_stream();

    // Consume the Stream using async/.await
    while let Some(item) = my_stream.next().await {
        match item {
            Ok(value) => println!("Stream received: {}", value),
            Err(err) => println!("Stream error: {:?}", err),
        }
    }
    println!("Observable to Stream example finished.");

    // You can also collect all items (if the stream is finite)
    let observable_to_collect = Shared::from_iter(vec![1, 2, 3]);
    let collected_items: Vec<_> = observable_to_collect.into_stream().collect().await;
    println!("Collected items: {:?}", collected_items);
}
```

## 3. Tokio Integration Practice: Scheduling Tasks

Ultimately, `rxRust`'s `Scheduler` implementations are designed to hand off tasks to the underlying asynchronous runtime (like Tokio) for execution.

By default, when using `SharedScheduler` in a native environment, rxRust is designed to integrate seamlessly with the Tokio runtime, leveraging `tokio::spawn` for task scheduling. This means if you have Tokio set up, `SharedScheduler` will automatically use it.

However, rxRust's `Scheduler` is a trait. If your project uses a different asynchronous runtime (e.g., `async-std`) or a custom event loop, you can implement your own `Scheduler` trait to integrate rxRust with your specific runtime. This allows you to tailor how tasks are executed and scheduled to your environment. For more details on implementing custom schedulers, refer to [Advanced Topics: Custom Context](../advanced/custom_context.md).

When working with `tokio` (or other async runtimes), `observe_on` and `subscribe_on` operators combined with `SharedScheduler` allow you to precisely control where tasks are executed.

### `observe_on` with Tokio

`observe_on` can be used to ensure that all downstream operations (including the final subscriber) execute on a specific scheduler. If you're in a `tokio` context, `SharedScheduler` will typically leverage `tokio::spawn` to offload tasks.

```rust,no_run
use rxrust::prelude::*;
use std::thread;

#[tokio::main]
async fn main() {
    println!("Starting Tokio observe_on example...");

    // Create an interval observable
    let source = Shared::interval(Duration::from_millis(100))
        .take(3); // Emit 0, 1, 2

    source
        .map(|v| {
            // This map might run on the creating thread or where the subscription is
            println!("Map on {:?}", thread::current().id());
            v * 2
        })
        .observe_on(SharedScheduler) // All emissions after this point will be processed via SharedScheduler
        .subscribe(|v| {
            // This subscriber will run on a Tokio worker thread
            println!("Subscriber on {:?}: Value = {}", thread::current().id(), v);
        });

    tokio::time::sleep(Duration::from_secs(1)).await; // Give time for operations to complete
    println!("Tokio observe_on example finished.");
}
```

### `subscribe_on` with Tokio

`subscribe_on` dictates where the *entire* observable chain, including the source observable's setup and initial emissions, will be executed. This is useful for moving long-running or blocking source operations to a background thread from the start.

```rust,no_run
use rxrust::prelude::*;
use std::thread;

#[tokio::main]
async fn main() {
    println!("Starting Tokio subscribe_on example...");

    // Create a simple observable
    let source = Shared::from_iter(0..3)
        .map(|v| {
            // This map will also be scheduled by subscribe_on's scheduler
            println!("Source Map on {:?}", thread::current().id());
            v * 10
        });

    source
        .subscribe_on(SharedScheduler) // The entire chain, including source setup, runs on SharedScheduler
        .subscribe(|v| {
            println!("Subscriber on {:?}: Value = {}", thread::current().id(), v);
        });

    tokio::time::sleep(Duration::from_millis(100)).await; // Give time for operations to complete
    println!("Tokio subscribe_on example finished.");
}
```
