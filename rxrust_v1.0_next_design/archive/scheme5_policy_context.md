# Refactoring Scheme 5: Policy via Observer Context (Final & Approved)

## Overview
This scheme achieves the ultimate decoupling of logic and policy. 
Instead of passing Policy as a runtime argument or a struct generic, we embed the Policy information into the **Observer Type System**.

Internal observers (like `MapObserver`) automatically propagate this policy context downstream. Stateful operators (like `MergeOp`) inspect this policy context to decide whether to use `Rc` or `Arc`.

### Key Innovations

1.  **Cleanest Operator Structs**: `MergeOp<S1, S2>` has NO generic `P`.
2.  **Cleanest Method Signature**: `actual_subscribe(observer)` has NO extra arguments.
3.  **Policy Propagation**: Achieved via the `QueryPolicy` trait on Observers.
4.  **Wrapper as Policy**: `Local<()>` and `Shared<()>` serve as the Policy markers.

## Core Definitions

### 1. SharedPolicy Trait
Defines the contract for a concurrency policy.

```rust
pub trait SharedPolicy: Default {
    type Rc<T>: RcDeref<Target = T> + From<T>;
}
```

### 2. QueryPolicy Trait (The Context Carrier)
Auxiliary trait implemented by Observers to carry the Policy type.

```rust
pub trait QueryPolicy {
    type P: SharedPolicy;
}
```

### 3. InnerObservable (The Core Logic)
Uses GAT to adapt `Unsub` based on the Observer's Policy.

```rust
pub trait InnerObservable {
    type Item;
    type Err;
    
    // GAT: Subscription type depends on the Observer's Policy
    type Unsub<O: QueryPolicy>: Subscription;

    // Standard signature!
    fn actual_subscribe<O>(self, observer: O) -> Self::Unsub<O>
    where O: Observer<Self::Item, Self::Err> + QueryPolicy;
}
```

### 4. ObservableExt (The Unified API)
Wraps user observers to inject the initial Policy.

```rust
pub trait ObservableExt: Sized {
    type Item;
    type Err;
    type Inner: InnerObservable<Item=Self::Item, Err=Self::Err>;
    type Wrapper<T: InnerObservable>;

    // ... helpers ...

    // --- Subscription Entry Point ---
    fn subscribe<O>(self, observer: O) 
    where
        O: Observer<Self::Item, Self::Err>,
        Self::Wrapper<()>: SharedPolicy,
    {
        let inner = self.into_inner();
        // Wrap user observer in PolicyWrapper to inject Policy P
        let wrapped = PolicyWrapper::new(observer, PhantomData::<Self::Wrapper<()>>);
        inner.actual_subscribe(wrapped);
    }

    // --- Operator: Merge (Clean!) ---
    fn merge<S>(self, other: S) -> Self::Wrapper<MergeOp<Self::Inner, S::Inner>>
    where
        S: ObservableExt<Item=Self::Item, Err=Self::Err>,
        Self::Wrapper<MergeOp<Self::Inner, S::Inner>>: ObservableExt
    {
        // No Policy generic needed here either!
        let op = MergeOp::new(self.into_inner(), other.into_inner());
        Self::wrap(op)
    }
}
```

## Internal Observers (e.g., MapObserver)
Must implement `QueryPolicy` to forward the context.

```rust
impl<O: QueryPolicy, F> QueryPolicy for MapObserver<O, F> {
    type P = O::P; // Forwarding
}
```

## Stateful Operators (e.g., MergeOp)
Extract Policy from `O::P`.

```rust
impl<S1, S2> InnerObservable for MergeOp<S1, S2> 
where S1: InnerObservable, S2: InnerObservable 
{
    // Unsub depends on O::P
    type Unsub<O: QueryPolicy> = MergeSubscription<<O::P as SharedPolicy>::Rc<MergeState>>;

    fn actual_subscribe<O>(self, observer: O) -> Self::Unsub<O>
    where O: Observer + QueryPolicy 
    {
        // Use O::P::Rc to create state
        let state = <O::P as SharedPolicy>::Rc::from(MergeState::new(...));
        MergeSubscription::new(state)
    }
}
```

## Execution Plan
1.  **Refactor `rc.rs`**: Ensure `SharedPolicy` supports `From<T>`.
2.  **Create `wrapper.rs`**: Define `Local` and `Shared`, implementing `SharedPolicy`.
3.  **Update `observer.rs`**: Define `QueryPolicy` trait and `PolicyWrapper`.
4.  **Update `observable.rs`**: Refactor `InnerObservable` and `ObservableExt`.
5.  **Refactor Operators**: Update all operators to implement `QueryPolicy` for their observers and adapt `InnerObservable` impls.

This is the definitive, clean architecture.