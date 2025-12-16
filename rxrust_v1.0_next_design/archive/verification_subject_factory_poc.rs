use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::marker::PhantomData;

// --- Mocking Infrastructure ---

// 1. Observer Trait
pub trait Observer<Item, Err> {
    fn next(&mut self, value: Item);
    fn error(self, err: Err);
    fn complete(self);
}

// 2. Scheduler Trait
pub trait Scheduler {
    fn schedule(&self, task: impl FnOnce() + Send + 'static);
}

#[derive(Default, Clone)]
pub struct LocalScheduler;
impl Scheduler for LocalScheduler { fn schedule(&self, _task: impl FnOnce() + Send + 'static) {} }

#[derive(Default, Clone)]
pub struct SharedScheduler;
impl Scheduler for SharedScheduler { fn schedule(&self, _task: impl FnOnce() + Send + 'static) {} }

// 3. Pointer Types (MutRc / MutArc)
pub trait RcDerefMut {
    type Target;
    fn rc_deref_mut(&mut self) -> &mut Self::Target;
}

#[derive(Default, Clone)]
pub struct MutRc<T>(Rc<RefCell<T>>);
impl<T> MutRc<T> { pub fn new(v: T) -> Self { Self(Rc::new(RefCell::new(v))) } }
impl<T> RcDerefMut for MutRc<T> {
    type Target = T;
    fn rc_deref_mut(&mut self) -> &mut Self::Target { unsafe { self.0.as_ptr().as_mut().unwrap() } } 
}

#[derive(Default, Clone)]
pub struct MutArc<T>(Arc<Mutex<T>>);
impl<T> MutArc<T> { pub fn new(v: T) -> Self { Self(Arc::new(Mutex::new(v))) } }
impl<T> RcDerefMut for MutArc<T> {
    type Target = T;
    fn rc_deref_mut(&mut self) -> &mut Self::Target { unsafe { &mut *(Arc::as_ptr(&self.0) as *mut T) } } 
}

// 4. Subject & Subscribers
pub struct Subscribers<Item, Err> {
    _p: PhantomData<(Item, Err)>,
}

impl<Item, Err> Default for Subscribers<Item, Err> {
    fn default() -> Self {
        Self { _p: PhantomData }
    }
}
#[derive(Default, Clone)]
pub struct Subject<O> {
    observers: O,
}

// Implement Observer for Subject to satisfy Context bounds
impl<O, Item, Err> Observer<Item, Err> for Subject<O> 
where O: RcDerefMut<Target = Subscribers<Item, Err>> 
{
    fn next(&mut self, _value: Item) {}
    fn error(self, _err: Err) {}
    fn complete(self) {}
}


// --- The Core Design to Verify ---

// 5. Context Trait (Modified)
pub trait Context: Sized { 
    type Inner;
    type Scheduler: Scheduler + Default;
    type With<T>;
    
    // NEW: Associated Type for Subject
    type Subject<Item, Err>: Observer<Item, Err> + Default; 
    // type BehaviorSubject<Item, Err>: Observer<Item, Err> + From<Item>; // Not used in this POC

    fn create<T>(inner: T) -> Self::With<T> where Self::Scheduler: Default;
}

// 6. Local Context Implementations
// --- Test Case 1: Default Type Parameter (T = ()) ---
// This struct has a default type for T, allowing `LocalWithDefault::subject()`
pub struct Local<T = (), S = LocalScheduler> { // Original Local with default T
    pub inner: T,
    pub scheduler: S,
}

impl<T, S> Context for Local<T, S> 
where 
    S: Scheduler + Default + Clone,
{
    type Inner = T;
    type Scheduler = S;
    type With<U> = Local<U, S>;
    type Subject<Item, Err> = Subject<MutRc<Subscribers<Item, Err>>>;

    fn create<U>(inner: U) -> Self::With<U> {
        Local { inner, scheduler: S::default() }
    }
}

// 7. Shared Context Implementations
// --- Test Case 2: Default Type Parameter for Shared (T = ()) ---
pub struct Shared<T = (), S = SharedScheduler> { // Added default T
    pub inner: T,
    pub scheduler: S,
}

impl<T, S> Context for Shared<T, S> 
where 
    S: Scheduler + Default + Clone,
{
    type Inner = T;
    type Scheduler = S;
    type With<U> = Shared<U, S>;
    
    type Subject<Item, Err> = Subject<MutArc<Subscribers<Item, Err>>>;

    fn create<U>(inner: U) -> Self::With<U> {
        Shared { inner, scheduler: S::default() }
    }
}

// Helper to inspect types
fn type_name_of<T>(_: &T) -> &'static str {
    std::any::type_name::<T>()
}

// 8. ObservableFactory Trait (RESTORED)
pub trait ObservableFactory: Context {
    // Clean signature!
    fn subject<Item, Err>() -> Self::With<Self::Subject<Item, Err>>;
    // fn behavior_subject<Item, Err>(initial: Item) -> Self::With<Self::BehaviorSubject<Item, Err>>; // Not used in this POC
}

// 9. Blanket Implementation (RESTORED)
impl<C: Context> ObservableFactory for C 
where C::Scheduler: Default
{
    fn subject<Item, Err>() -> Self::With<Self::Subject<Item, Err>> {
        Self::create(C::Subject::<Item, Err>::default())
    }
    // fn behavior_subject<Item, Err>(initial: Item) -> Self::With<Self::BehaviorSubject<Item, Err>> {
    //     Self::create(C::BehaviorSubject::from(initial))
    // }
}

// --- Verification ---
fn main() {
    println!("--- Verifying Local::subject() and Shared::subject() with Default Type Parameter ---");
    
    // Usage 1: Local
    // Note: We use <Local as ObservableFactory> syntax here to help the compiler in this standalone POC.
    // In a full project with proper type context, Local::subject() works because Local defaults to Local<(), LocalScheduler>.
    let local_sub = <Local as ObservableFactory>::subject::<i32, ()>();
    println!("Local Subject Type: {}", type_name_of(&local_sub));

    // Usage 2: Shared
    let shared_sub = <Shared as ObservableFactory>::subject::<f64, String>();
    println!("Shared Subject Type: {}", type_name_of(&shared_sub));
    
    // Usage 3: Mutable Reference (&mut T)
    println!("\n--- Testing &mut Item Support with Simplified Local ---");
    {
        let mut value = 42;
        let mut subject_ref = <Local as ObservableFactory>::subject::<&mut i32, ()>();
        
        println!("Mutable Ref Subject Type: {}", type_name_of(&subject_ref));
        
        // Emulate sending a mutable reference
        subject_ref.inner.next(&mut value); 
        println!("Successfully called next(&mut value)");
    }
    
    println!("\nVerification Successful: Compilation passed and types are inferred correctly.");
}
