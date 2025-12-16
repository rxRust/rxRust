use std::cell::RefCell;
use std::rc::Rc;

// ==========================================
// 0. Context (Standard)
// ==========================================
pub trait Context: Sized {
  type Inner;
  type Rc<T>: From<T> + Clone;
  type With<T: 'static>: Context<Inner = T>;
  fn into_inner(self) -> Self::Inner;
  fn pack<T: 'static>(inner: T) -> Self::With<T>;
}

// Local Context
#[derive(Default, Clone, Copy)]
pub struct Local<T>(pub T);
pub struct MutRc<T>(Rc<RefCell<T>>);
impl<T> Clone for MutRc<T> { fn clone(&self) -> Self { MutRc(self.0.clone()) } }
impl<T> From<T> for MutRc<T> { fn from(v: T) -> Self { Self(Rc::new(RefCell::new(v))) } }

impl<T: 'static> Context for Local<T> {
  type Inner = T;
  type Rc<U> = MutRc<U>;
  type With<U: 'static> = Local<U>;
  fn into_inner(self) -> T { self.0 }
  fn pack<U: 'static>(inner: U) -> Local<U> { Local(inner) }
}

// ==========================================
// 1. Observer
// ==========================================
pub trait Observer<Item, Err> {
  fn next(&mut self, v: Item);
}

// Discarding Observer (Universal)
pub struct DiscardingObserver;
impl<I, E> Observer<I, E> for DiscardingObserver {
    fn next(&mut self, _v: I) {}
}

// ==========================================
// 2. CoreObservable (With Generic Params, NO Assoc Types)
// ==========================================

// Item and Err are generic parameters now!
pub trait CoreObservable<Item, Err, O> {
    fn subscribe(self, observer: O);
}

// ==========================================
// 3. The Auxiliary Trait (Captures Types)
// ==========================================

pub trait ObservableExt {
    type Item;
    type Err;
}

// THE MAGIC BRIDGE
// We try to implement ObservableExt automatically for anything that implements CoreObservable
// for the DiscardingObserver.
impl<T, I, E> ObservableExt for T
where
    T: CoreObservable<I, E, DiscardingObserver>
{
    type Item = I;
    type Err = E;
}

// ==========================================
// 4. Implementations
// ==========================================

// --- Of ---
pub struct Of<T>(pub T);

// Impl for generic O
impl<O, T> CoreObservable<T, (), O> for Of<T>
where 
    T: Clone + 'static,
    O: Observer<T, ()>
{
    fn subscribe(self, mut observer: O) {
        observer.next(self.0);
    }
}

// --- Map ---
pub struct Map<S, F> { source: S, func: F }
pub struct MapObserver<O, F> { observer: O, func: F }

impl<O, F, In, Out, Err> Observer<In, Err> for MapObserver<O, F>
where
  O: Observer<Out, Err>,
  F: FnMut(In) -> Out,
{
  fn next(&mut self, v: In) {
    let mapped = (self.func)(v);
    self.observer.next(mapped);
  }
}

// Map Implementation
// Note: We need to specify I, E for the Source
impl<S, F, O, In, Out, Err> CoreObservable<Out, Err, O> for Map<S, F>
where
    S: CoreObservable<In, Err, MapObserver<O, F>>, // Source produces In
    F: FnMut(In) -> Out,
    O: Observer<Out, Err>,
{
    fn subscribe(self, observer: O) {
        let map_observer = MapObserver { observer, func: self.func };
        self.source.subscribe(map_observer)
    }
}

// ==========================================
// 5. Observable Facade
// ==========================================

pub trait Observable: Context 
where 
    Self::Inner: ObservableExt // Use the Aux trait
{
    type Item = <Self::Inner as ObservableExt>::Item;
    type Err = <Self::Inner as ObservableExt>::Err;

    // Standard subscribe...
    fn subscribe<O>(self, observer: O) 
    where
        // Here we need to link types back to CoreObservable
        Self::Inner: CoreObservable<Self::Item, Self::Err, Self::With<O>>,
        O: 'static 
    {
        let core = self.into_inner();
        let wrapped = Self::pack(observer);
        core.subscribe(wrapped)
    }

    // Map...
    fn map<F, Out>(self, f: F) -> Self::With<Map<Self::Inner, F>>
    where
        F: FnMut(Self::Item) -> Out + 'static,
        Self::Inner: 'static,
        // Ensure result is ObservableExt
        Map<Self::Inner, F>: ObservableExt<Item=Out, Err=Self::Err>
    {
        let core = self.into_inner();
        Self::pack(Map { source: core, func: f })
    }
}

// Blanket impl
impl<T> Observable for T
where
    T: Context,
    T::Inner: ObservableExt, // This requires the Magic Bridge to work
{}

// ==========================================
// 6. Verification
// ==========================================
fn main() {
    println!("Testing Generic Parameters Approach");
    
    // Test inference
    fn check_inference<T: ObservableExt<Item=i32>>(_: T) {}
    
    let of_stream = Of(10); // Should be ObservableExt<Item=i32>
    check_inference(of_stream);
    
    // Test Map
    // Map<Of<i32>, ...> should be ObservableExt<Item=i32> (if mapped to i32)
    let map_stream = Map { source: Of(10), func: |x: i32| x * 2 };
    check_inference(map_stream);
}
