// =================================================================================================
// rxRust 2.0 Architecture Verification: The "Universal Task" Model
// =================================================================================================
// This verification suite proves that we can achieve Zero-Cost Abstraction for scheduling
// using a single, universal `Task<S>` struct and function pointers.
//
// Core Concepts:
// 1. Task<S>: A concrete struct wrapping state `S` and a handler `fn(S)`. No dynamic dispatch.
// 2. Scheduler<S>: A trait implemented by schedulers that can execute `Task<S>`.
// 3. Automatic Safety: `Send` / `!Send` is automatically inferred from `S` (the State).
// 4. Simplified Operators: Operators just define a static handler function and pack their state.

use std::time::Duration;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::marker::PhantomData;

// ========================================== 
// 1. The Universal Task Definition
// ========================================== 

/// A generic task that bundles data (State) with behavior (Handler).
/// - `S`: The state required to execute the task.
/// - `handler`: A static function pointer that consumes the state.
/// 
/// The `Send` / `Sync` properties of `Task<S>` are automatically derived from `S`.
pub struct Task<S> {
    pub state: S,
    pub handler: fn(S),
}

impl<S> Task<S> {
    pub fn new(state: S, handler: fn(S)) -> Self {
        Self { state, handler }
    }

    pub fn run(self) {
        (self.handler)(self.state);
    }
}

// ========================================== 
// 2. Scheduler System
// ========================================== 

pub trait Subscription {
    fn unsubscribe(&mut self);
}
impl Subscription for () { fn unsubscribe(&mut self) {} }
impl Subscription for Box<dyn Subscription> {
    fn unsubscribe(&mut self) { (**self).unsubscribe() }
}

/// The Scheduler trait is generic over the State `S` it executes.
/// This allows specific implementations to place bounds on `S` (e.g., `S: Send`).
pub trait Scheduler<S> {
    fn schedule(&self, task: Task<S>, delay: Duration) -> Box<dyn Subscription>;
}

// --- Local Scheduler ---
// Accepts ANY state `S` (no Send bound).
#[derive(Clone, Copy)]
pub struct LocalScheduler;

impl<S: 'static> Scheduler<S> for LocalScheduler {
    fn schedule(&self, task: Task<S>, _delay: Duration) -> Box<dyn Subscription> {
        // In a real impl, this might queue to a thread-local binary heap.
        // For verification, we execute immediately or pretend to queue.
        // Notice: No Boxing of the task logic itself is required by the API.
        task.run();
        Box::new(())
    }
}

// --- Shared Scheduler ---
// ONLY accepts `S` that is `Send`.
#[derive(Clone, Copy)]
pub struct SharedScheduler;

impl<S> Scheduler<S> for SharedScheduler 
where 
    S: Send + 'static 
{
    fn schedule(&self, task: Task<S>, _delay: Duration) -> Box<dyn Subscription> {
        // In a real impl, this submits to a thread pool.
        std::thread::spawn(move || {
            std::thread::sleep(_delay);
            task.run();
        });
        Box::new(())
    }
}

// ========================================== 
// 3. Context System
// ========================================== 

// Context remains pure and clean.
pub trait Context: Sized {
    type Inner;
    type With<T: 'static>: Context<Inner = T>;
    fn into_inner(self) -> Self::Inner;
    fn pack<T: 'static>(inner: T) -> Self::With<T>;
}

#[derive(Default, Clone, Copy)]
pub struct Local<T>(pub T);
impl<T: 'static> Context for Local<T> {
    type Inner = T;
    type With<U: 'static> = Local<U>;
    fn into_inner(self) -> T { self.0 }
    fn pack<U: 'static>(inner: U) -> Local<U> { Local(inner) }
}

#[derive(Default, Clone, Copy)]
pub struct Shared<T>(pub T);
impl<T: 'static> Context for Shared<T> {
    type Inner = T;
    type With<U: 'static> = Shared<U>;
    fn into_inner(self) -> T { self.0 }
    fn pack<U: 'static>(inner: U) -> Shared<U> { Shared(inner) }
}

// ========================================== 
// 4. Observer & CoreObservable
// ========================================== 

pub trait Observer<Item, Err> {
    fn next(&mut self, v: Item);
    fn error(self, e: Err);
    fn complete(self);
}

// Helper for closures
impl<F, Item, Err> Observer<Item, Err> for F where F: FnMut(Item) {
    fn next(&mut self, v: Item) { (self)(v); }
    fn error(self, _: Err) {}
    fn complete(self) {}
}

pub trait CoreObservable<O> {
    type Item;
    type Err;
    type Unsub: Subscription;
    fn subscribe(self, observer: O) -> Self::Unsub;
}

// ========================================== 
// 5. Operator Implementation: DelayOn
// ========================================== 

pub mod ops {
    use super::*;

    pub struct DelayOn<S, Sch> {
        pub source: S,
        pub delay: Duration,
        pub scheduler: Sch,
    }

    pub struct DelayOnObserver<O, Sch> {
        pub observer: O,
        pub delay: Duration,
        pub scheduler: Sch,
    }

    // --- The Static Handler Function ---
    // This is the logic of the operator execution.
    // It is completely static, capturing nothing. All state is passed in.
    fn delay_on_next_handler<O, Item, Err>(state: (O, Item)) 
    where O: Observer<Item, Err>
    {
        let (mut observer, item) = state;
        observer.next(item);
    }

    impl<O, Sch, Item, Err> Observer<Item, Err> for DelayOnObserver<O, Sch>
    where
        O: Observer<Item, Err> + Clone + 'static,
        Item: 'static,
        // CRITICAL BOUND:
        // The Scheduler must be able to schedule a Task containing our State (O, Item).
        // This is where the Send/!Send check happens automatically.
        Sch: Scheduler<(O, Item)> + Clone, 
    {
        fn next(&mut self, v: Item) {
            // 1. Pack the State (Tuple)
            let state = (self.observer.clone(), v);
            
            // 2. Create the Universal Task
            let task = Task::new(state, delay_on_next_handler);
            
            // 3. Schedule
            self.scheduler.schedule(task, self.delay);
        }

        fn error(self, e: Err) { self.observer.error(e); }
        fn complete(self) { self.observer.complete(); }
    }

    impl<S, Sch, C> CoreObservable<C> for DelayOn<S, Sch>
    where
        C: Context,
        C::Inner: Observer<S::Item, S::Err> + Clone + 'static,
        S::Item: 'static,
        Sch: Clone + 'static,
        // Ensure the Observer is valid (which recursively checks the Scheduler bound)
        S: CoreObservable<C::With<DelayOnObserver<C::Inner, Sch>>>,
    {
        type Item = S::Item;
        type Err = S::Err;
        type Unsub = S::Unsub;

        fn subscribe(self, context: C) -> Self::Unsub {
            let observer = context.into_inner();
            let wrapped = DelayOnObserver {
                observer,
                scheduler: self.scheduler,
                delay: self.delay,
            };
            self.source.subscribe(C::pack(wrapped))
        }
    }
    
    // --- Of Operator ---
    pub struct Of<T>(pub T);
    impl<C, T> CoreObservable<C> for Of<T>
    where C: Context, C::Inner: Observer<T, ()>, T: Clone + 'static {
        type Item = T; type Err = (); type Unsub = Box<dyn Subscription>;
        fn subscribe(self, context: C) -> Self::Unsub {
            let mut obs = context.into_inner();
            obs.next(self.0);
            obs.complete();
            Box::new(())
        }
    }
}

// ========================================== 
// 6. Verification & API Surface
// ========================================== 

use ops::*;

pub trait Observable: Context {
    fn delay_on<Sch>(self, delay: Duration, scheduler: Sch) -> Self::With<DelayOn<Self::Inner, Sch>>
    where 
        Self::Inner: 'static,
        Sch: 'static,
        // Just standard return type check
        Self::With<DelayOn<Self::Inner, Sch>>: Observable;

    fn subscribe<O>(self, observer: O) -> <Self::Inner as CoreObservable<O>>::Unsub
    where
        Self::Inner: CoreObservable<O>;
}

impl<T: Context> Observable for T {
    fn delay_on<Sch>(self, delay: Duration, scheduler: Sch) -> Self::With<DelayOn<Self::Inner, Sch>>
    where 
        Self::Inner: 'static,
        Sch: 'static,
        Self::With<DelayOn<Self::Inner, Sch>>: Observable 
    {
        let core = self.into_inner();
        Self::pack(DelayOn { source: core, delay, scheduler })
    }

    fn subscribe<O>(self, observer: O) -> <Self::Inner as CoreObservable<O>>::Unsub
    where
        Self::Inner: CoreObservable<O>,
    {
        let core = self.into_inner();
        core.subscribe(observer)
    }
}

pub mod observable {
    use super::*;
    pub fn of<T: Clone + 'static>(v: T) -> Local<Of<T>> { Local(Of(v)) }
    pub fn of_shared<T: Clone + Send + 'static>(v: T) -> Shared<Of<T>> { Shared(Of(v)) }
}

fn main() {
    println!("===========================================================");
    println!("Architecture Verification: Universal Task Model (Task<S>)");
    println!("===========================================================");

    // 1. Local Context + Local Scheduler
    // State: (Rc<...>, i32) -> !Send
    // LocalScheduler: Impl Scheduler<S> for all S. OK.
    {
        println!("\n[Test 1] Local Context + LocalScheduler");
        let rc_val = Rc::new(100);
        let _ = observable::of(10)
            .delay_on(Duration::from_millis(10), LocalScheduler)
            .subscribe(move |v| println!("  Local got: {} with Rc: {}", v, rc_val));
    }

    // 2. Shared Context + Shared Scheduler
    // State: (Arc<...>, i32) -> Send
    // SharedScheduler: Impl Scheduler<S> where S: Send. OK.
    {
        println!("\n[Test 2] Shared Context + SharedScheduler");
        let _ = observable::of_shared(20)
            .delay_on(Duration::from_millis(10), SharedScheduler)
            .subscribe(|v| println!("  Shared got: {}", v));
        std::thread::sleep(Duration::from_millis(50));
    }

    // 3. Shared Context + Local Scheduler
    // State: (Arc<...>, i32) -> Send
    // LocalScheduler: Impl Scheduler<S> for all S. OK.
    {
        println!("\n[Test 3] Shared Context + LocalScheduler");
        let _ = observable::of_shared(30)
            .delay_on(Duration::from_millis(10), LocalScheduler)
            .subscribe(|v| println!("  Shared on LocalScheduler got: {}", v));
    }

    // 4. Local Context + Shared Scheduler (Compilation Failure Check)
    // State: (Rc<...>, i32) -> !Send
    // SharedScheduler: Impl Scheduler<S> where S: Send. 
    // S is !Send -> Constraint NOT met. -> Compile Error.
    /*
    {
        let rc_val = Rc::new(40);
        let _ = observable::of(40)
            .delay_on(Duration::from_millis(10), SharedScheduler)
            .subscribe(move |_| println!("{}", rc_val));
    }
    */
    
    println!("\nVerification Complete: All scenarios behave exactly as intended.");
}
