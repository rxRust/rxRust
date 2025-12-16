use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

// ==========================================
// 0. The Unified Trait: Context (Scheme 8/10)
//    Wrapper + Policy + Factory
// ==========================================

pub trait Context: Sized {
    type Inner;

    // --- Policy Information ---
    type Rc<T>: From<T> + Clone;

    // --- Wrapper Factory (GAT) ---
    // Allows creating a NEW context of the SAME family.
    type With<T>: Context<Inner = T>;

    // --- Wrapper Behavior ---
    fn into_inner(self) -> Self::Inner;
    fn pack<T>(inner: T) -> Self::With<T>;
}

// --- Local Implementation ---
#[derive(Default, Clone, Copy)]
pub struct Local<T>(pub T);

pub struct MutRc<T>(Rc<RefCell<T>>);
impl<T> Clone for MutRc<T> { fn clone(&self) -> Self { MutRc(self.0.clone()) } }
impl<T> From<T> for MutRc<T> { fn from(v: T) -> Self { Self(Rc::new(RefCell::new(v))) } }

impl<T> Context for Local<T> {
    type Inner = T;
    type Rc<U> = MutRc<U>;
    type With<U> = Local<U>;
    fn into_inner(self) -> T { self.0 }
    fn pack<U>(inner: U) -> Local<U> { Local(inner) }
}

// --- Shared Implementation ---
#[derive(Default, Clone, Copy)]
pub struct Shared<T>(pub T);

pub struct MutArc<T>(Arc<Mutex<T>>);
impl<T> Clone for MutArc<T> { fn clone(&self) -> Self { MutArc(self.0.clone()) } }
impl<T> From<T> for MutArc<T> { fn from(v: T) -> Self { Self(Arc::new(Mutex::new(v))) } }

impl<T> Context for Shared<T> {
    type Inner = T;
    type Rc<U> = MutArc<U>;
    type With<U> = Shared<U>;
    fn into_inner(self) -> T { self.0 }
    fn pack<U>(inner: U) -> Shared<U> { Shared(inner) }
}

// ==========================================
// 1. Observer & Subscription
// ==========================================

pub trait Observer<Item, Err> {
    fn next(&mut self, v: Item);
}

impl<I, E> Observer<I, E> for () {
    fn next(&mut self, _: I) {}
}

pub trait Subscription {
    fn unsubscribe(&mut self);
}

impl Subscription for () {
    fn unsubscribe(&mut self) {}
}

pub struct MergeSubscription<T>(T);
impl<T> Subscription for MergeSubscription<T> {
    fn unsubscribe(&mut self) {}
}

// ==========================================
// 2. ObservableCore (Logic Kernel)
// ==========================================

pub trait ObservableCore {
    type Item;
    type Err;

    type Unsub<C: Context>: Subscription;

    fn subscribe_core<C>(self, observer: C) -> Self::Unsub<C>
    where
        C: Context,
        C::Inner: Observer<Self::Item, Self::Err>;
}

// --- Of (Pure Logic) ---
pub struct Of<T>(T);
impl<T> Of<T> { pub fn new(v: T) -> Self { Self(v) } }

impl<T> ObservableCore for Of<T> {
    type Item = T;
    type Err = ();
    type Unsub<C: Context> = ();

    fn subscribe_core<C>(self, observer: C) -> ()
    where
        C: Context,
        C::Inner: Observer<T, ()>,
    {
        let mut inner = observer.into_inner();
        inner.next(self.0);
    }
}

// --- Map (Pure Logic) ---
pub struct Map<S, F> {
    source: S,
    func: F,
}

pub struct MapObserver<O, F> {
    observer: O,
    func: F,
}

impl<O, F, Item, Out, Err> Observer<Item, Err> for MapObserver<O, F>
where
    O: Observer<Out, Err>,
    F: FnMut(Item) -> Out,
{
    fn next(&mut self, v: Item) {
        let mapped = (self.func)(v);
        self.observer.next(mapped);
    }
}

impl<S, F, B> ObservableCore for Map<S, F>
where
    S: ObservableCore,
    F: FnMut(S::Item) -> B,
{
    type Item = B;
    type Err = S::Err;
    type Unsub<C: Context> = S::Unsub<C::With<MapObserver<C::Inner, F>>>;

    fn subscribe_core<C>(self, observer: C) -> Self::Unsub<C>
    where
        C: Context,
        C::Inner: Observer<Self::Item, Self::Err>,
    {
        let inner_obs = observer.into_inner();
        let map_observer = MapObserver {
            observer: inner_obs,
            func: self.func,
        };
        let wrapped = C::pack(map_observer);
        self.source.subscribe_core(wrapped)
    }
}

// --- Merge (Pure Logic) ---
pub struct Merge<S1, S2> {
    s1: S1,
    s2: S2,
}
pub struct MergeState {}

impl<S1, S2> ObservableCore for Merge<S1, S2>
where
    S1: ObservableCore,
    S1::Item: Clone,
    S2: ObservableCore<Item = S1::Item, Err = S1::Err>,
{
    type Item = S1::Item;
    type Err = S1::Err;
    type Unsub<C: Context> = MergeSubscription<C::Rc<MergeState>>;

    fn subscribe_core<C>(self, observer: C) -> Self::Unsub<C>
    where
        C: Context,
        C::Inner: Observer<Self::Item, Self::Err>,
    {
        let state = C::Rc::from(MergeState {});
        let _inner = observer.into_inner();
        MergeSubscription(state)
    }
}

// ==========================================
// 3. Observable (User API - Ultimate Simplification)
//    No GATs, No Associated Types!
// ==========================================

pub trait Observable: Context
where
    Self::Inner: ObservableCore,
{
    // No explicit Item/Err types needed!
    // They are implicitly provided by Self::Inner::Item/Err

    fn subscribe<O>(self, observer: O) -> <Self::Inner as ObservableCore>::Unsub<Self::With<O>>
    where
        O: Observer<<Self::Inner as ObservableCore>::Item, <Self::Inner as ObservableCore>::Err>,
    {
        let core = self.into_inner();
        let wrapped = Self::pack(observer);
        core.subscribe_core(wrapped)
    }

    fn map<B, F>(self, f: F) -> Self::With<Map<Self::Inner, F>>
    where
        F: FnMut(<Self::Inner as ObservableCore>::Item) -> B,
        // Ensure the returned type is also a valid ObservableCore so it can be wrapped
        Map<Self::Inner, F>: ObservableCore,
    {
        let core = self.into_inner();
        let op = Map { source: core, func: f };
        Self::pack(op)
    }

    fn merge<S>(self, other: S) -> Self::With<Merge<Self::Inner, S::Inner>>
    where
        S: Observable, // Other must be an Observable (Context)
        // Ensure Item types match
        <Self::Inner as ObservableCore>::Item: Clone,
        S::Inner: ObservableCore<
            Item = <Self::Inner as ObservableCore>::Item,
            Err = <Self::Inner as ObservableCore>::Err
        >,
        // Ensure Merge returns a valid Core
        Merge<Self::Inner, S::Inner>: ObservableCore,
    {
        let core1 = self.into_inner();
        let core2 = other.into_inner();
        let op = Merge { s1: core1, s2: core2 };
        Self::pack(op)
    }
}

// ==========================================
// 4. Implementations (Zero-Boilerplate)
// ==========================================

// Automatic implementation for any Context wrapping a Core!
impl<T> Observable for T
where
    T: Context,
    T::Inner: ObservableCore,
{
    // No associated types to define!
    // Methods are inherited from default impl.
}

// ==========================================
// 5. Verification
// ==========================================

fn main() {
    println!("Scheme 10: Ultimate Simplification (Implied Context)");

    // 1. Local Pipeline
    let l1 = Local(Of::new(1));
    let l2 = Local(Of::new(2));
    
    // Map
    let mapped = l1.map(|x| x * 2);
    
    // Merge
    let merged = mapped.merge(l2);
    
    // Subscribe
    merged.subscribe(());
    println!("Local pipeline works");

    // 2. Shared Pipeline (Mock)
    // let s1 = Shared(Of::new(10));
    // let s2 = Shared(Of::new(20));
    // s1.merge(s2).subscribe(());
}