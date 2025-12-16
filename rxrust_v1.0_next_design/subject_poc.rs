// =================================================================================================
// Subject Context POC (Final Design: Subject<O>, HRTB, No MutRef, Allow Warning)
// =================================================================================================

#[allow(coherence_leak_check)]
// 允许这个警告，因为 &mut T 永远不会实现 Clone，所以冲突是理论上的，实际上不会发生
use smallvec::SmallVec;
use std::cell::{RefCell, RefMut};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};

// ==========================================
// 0. Utilities & Traits
// ==========================================

pub trait Observer<Item, Err> {
  fn next(&mut self, v: Item);
  fn error(&mut self, e: Err);
  fn complete(&mut self);
}

// Box<O> 透传实现
impl<Item, Err, O: ?Sized> Observer<Item, Err> for Box<O>
where
  O: Observer<Item, Err>,
{
  fn next(&mut self, v: Item) {
    (**self).next(v);
  }
  fn error(&mut self, e: Err) {
    (**self).error(e);
  }
  fn complete(&mut self) {
    (**self).complete();
  }
}

pub trait Subscription {
  fn unsubscribe(&mut self);
  fn is_closed(&self) -> bool;
}

impl Subscription for () {
  fn unsubscribe(&mut self) {}
  fn is_closed(&self) -> bool {
    true
  }
}

// --- RcDerefMut Trait ---
pub trait RcDerefMut {
  type Target;
  type Guard<'a>: DerefMut<Target = Self::Target>
  where
    Self: 'a;
  fn rc_deref_mut(&self) -> Self::Guard<'_>;
}

// --- MutRc (Local) ---
pub struct MutRc<T>(Rc<RefCell<T>>);
impl<T> Clone for MutRc<T> {
  fn clone(&self) -> Self {
    MutRc(self.0.clone())
  }
}
impl<T> From<T> for MutRc<T> {
  fn from(v: T) -> Self {
    Self(Rc::new(RefCell::new(v)))
  }
}
impl<T> RcDerefMut for MutRc<T> {
  type Target = T;
  type Guard<'a>
    = RefMut<'a, T>
  where
    T: 'a;
  fn rc_deref_mut(&self) -> Self::Guard<'_> {
    self.0.borrow_mut()
  }
}

// --- MutArc (Shared) ---
pub struct MutArc<T>(Arc<Mutex<T>>);
impl<T> Clone for MutArc<T> {
  fn clone(&self) -> Self {
    MutArc(self.0.clone())
  }
}
impl<T> From<T> for MutArc<T> {
  fn from(v: T) -> Self {
    Self(Arc::new(Mutex::new(v)))
  }
}
impl<T> RcDerefMut for MutArc<T> {
  type Target = T;
  type Guard<'a>
    = MutexGuard<'a, T>
  where
    T: 'a;
  fn rc_deref_mut(&self) -> Self::Guard<'_> {
    self.0.lock().unwrap()
  }
}

// ==========================================
// 1. Context Trait
// ==========================================

pub trait Context: Sized {
  type Rc<T>: From<T> + Clone + RcDerefMut<Target = T>;
  type With<T>: DerefMut<Target = T>;

  fn create<T>(inner: T) -> Self::With<T>;
}

// ==========================================
// 2. CoreObservable Trait (With GAT)
// ==========================================

pub trait CoreObservable<O> {
  type Item<'a>
  where
    Self: 'a; // 引入 GAT
  type Err;
  type Unsub: Subscription;

  fn subscribe(self, observer: O) -> Self::Unsub;
}

// ==========================================
// 3. Local & Shared Implementations
// ==========================================

#[derive(Default, Clone)]
pub struct Local<T>(pub T);
impl<T> Deref for Local<T> {
  type Target = T;
  fn deref(&self) -> &T {
    &self.0
  }
}
impl<T> DerefMut for Local<T> {
  fn deref_mut(&mut self) -> &mut T {
    &mut self.0
  }
}

impl Context for Local<()> {
  type Rc<T> = MutRc<T>;
  type With<T> = Local<T>;
  fn create<T>(inner: T) -> Self::With<T> {
    Local(inner)
  }
}

#[derive(Default, Clone)]
pub struct Shared<T>(pub T);
impl<T> Deref for Shared<T> {
  type Target = T;
  fn deref(&self) -> &T {
    &self.0
  }
}
impl<T> DerefMut for Shared<T> {
  fn deref_mut(&mut self) -> &mut T {
    &mut self.0
  }
}

impl Context for Shared<()> {
  type Rc<T> = MutArc<T>;
  type With<T> = Shared<T>;
  fn create<T>(inner: T) -> Self::With<T> {
    Shared(inner)
  }
}

// ==========================================
// 4. Subject Definition & Subscription
// ==========================================

pub struct Subscribers<Ob> {
  next_id: usize,
  list: SmallVec<[(usize, Ob); 2]>,
}

impl<Ob> Default for Subscribers<Ob> {
  fn default() -> Self {
    Self { next_id: 0, list: SmallVec::new() }
  }
}

impl<Ob> Subscribers<Ob> {
  fn add(&mut self, observer: Ob) -> usize {
    let id = self.next_id;
    self.next_id += 1;
    self.list.push((id, observer));
    id
  }
  fn remove(&mut self, id: usize) {
    if let Some(pos) = self.list.iter().position(|(i, _)| *i == id) {
      self.list.remove(pos);
    }
  }
}

pub struct SubjectSubscription<O, C>
where
  C: Context,
{
  pub(crate) observers: Option<C::Rc<Subscribers<O>>>,
  pub(crate) id: usize,
}

impl<O, C> Subscription for SubjectSubscription<O, C>
where
  C: Context,
{
  fn unsubscribe(&mut self) {
    if let Some(observers) = self.observers.take() {
      let mut guard = observers.rc_deref_mut();
      guard.remove(self.id);
    }
  }
  fn is_closed(&self) -> bool {
    self.observers.is_none()
  }
}

pub struct Subject<O, C>
where
  C: Context,
{
  pub observers: C::Rc<Subscribers<O>>,
}

impl<O, C> Clone for Subject<O, C>
where
  C: Context,
{
  fn clone(&self) -> Self {
    Self { observers: self.observers.clone() }
  }
}

impl<O, C> Default for Subject<O, C>
where
  C: Context,
{
  fn default() -> Self {
    Self { observers: C::Rc::from(Subscribers::<O>::default()) }
  }
}

impl<O, C> Subject<O, C>
where
  C: Context,
{
  pub fn subscribe<SubO>(&self, observer: SubO) -> SubjectSubscription<O, C>
  where
    SubO: Into<O>,
  {
    let mut guard = self.observers.rc_deref_mut();
    let id = guard.add(observer.into());

    SubjectSubscription { observers: Some(self.observers.clone()), id }
  }
}

// Implement CoreObservable for Subject (Standard Case: Box<dyn Observer>) - UPDATED FOR GAT
impl<Item, Err, C, SubO> CoreObservable<SubO> for Subject<Box<dyn Observer<Item, Err>>, C>
where
  C: Context,
  SubO: Into<Box<dyn Observer<Item, Err>>>,
{
  type Item<'a>
    = Item
  where
    Self: 'a; // For owned types, GAT is just the type
  type Err = Err;
  type Unsub = SubjectSubscription<Box<dyn Observer<Item, Err>>, C>;

  fn subscribe(self, observer: SubO) -> Self::Unsub {
    // CoreObservable::subscribe consumes self, so we clone it to call the inherent subscribe method.
    Subject::subscribe(&self.clone(), observer)
  }
}

// Implement CoreObservable for Subject (Standard Case: Box<dyn Observer> + Send) - NEW IMPL FOR GAT
impl<Item, Err, C, SubO> CoreObservable<SubO> for Subject<Box<dyn Observer<Item, Err> + Send>, C>
where
  C: Context,
  SubO: Into<Box<dyn Observer<Item, Err> + Send>>,
{
  type Item<'a>
    = Item
  where
    Self: 'a;
  type Err = Err;
  type Unsub = SubjectSubscription<Box<dyn Observer<Item, Err> + Send>, C>;

  fn subscribe(self, observer: SubO) -> Self::Unsub {
    Subject::subscribe(&self.clone(), observer)
  }
}

// Implement CoreObservable for Subject (HRTB MutRef Case) - NEW IMPL FOR GAT
impl<T, Err, C, SubO> CoreObservable<SubO> for Subject<Box<dyn for<'a> Observer<&'a mut T, Err>>, C>
where
  C: Context,
  SubO: Into<Box<dyn for<'a> Observer<&'a mut T, Err>>>,
{
  type Item<'a>
    = &'a mut T
  where
    Self: 'a;
  type Err = Err;
  type Unsub = SubjectSubscription<Box<dyn for<'a> Observer<&'a mut T, Err>>, C>;

  fn subscribe(self, observer: SubO) -> Self::Unsub {
    Subject::subscribe(&self.clone(), observer)
  }
}

// Implement CoreObservable for Subject (HRTB MutRef Case + Send) - NEW IMPL FOR GAT

impl<T, Err, C, SubO> CoreObservable<SubO>
  for Subject<Box<dyn for<'a> Observer<&'a mut T, Err> + Send>, C>
where
  C: Context,
  SubO: Into<Box<dyn for<'a> Observer<&'a mut T, Err> + Send>>,
{
  type Item<'a>
    = &'a mut T
  where
    Self: 'a;

  type Err = Err;

  type Unsub = SubjectSubscription<Box<dyn for<'a> Observer<&'a mut T, Err> + Send>, C>;

  fn subscribe(self, observer: SubO) -> Self::Unsub {
    Subject::subscribe(&self.clone(), observer)
  }
}

// ==========================================

// 5. ObservableFactory (Updated for GAT)

// ==========================================

pub trait ObservableFactory: Context {
  // GAT Item 需要一个生命周期参数 'stream

  type BoxedObserver<'stream, Item, Err>;

  type BoxedMutRefObserver<'stream, Item, Err>;

  fn subject<'stream, Item, Err>()
  -> Self::With<Subject<Self::BoxedObserver<'stream, Item, Err>, Self>>
  where
    Self: Sized;

  fn mut_ref_subject<'stream, Item, Err>()
  -> Self::With<Subject<Self::BoxedMutRefObserver<'stream, Item, Err>, Self>>
  where
    Self: Sized;
}

impl ObservableFactory for Local<()> {
  type BoxedObserver<'stream, Item, Err> = Box<dyn Observer<Item, Err> + 'stream>;

  type BoxedMutRefObserver<'stream, Item, Err> =
    Box<dyn for<'a> Observer<&'a mut Item, Err> + 'stream>;

  fn subject<'stream, Item, Err>()
  -> Self::With<Subject<Self::BoxedObserver<'stream, Item, Err>, Self>> {
    Local::create(Subject::default())
  }

  fn mut_ref_subject<'stream, Item, Err>()
  -> Self::With<Subject<Self::BoxedMutRefObserver<'stream, Item, Err>, Self>> {
    Local::create(Subject::default())
  }
}

impl ObservableFactory for Shared<()> {
  type BoxedObserver<'stream, Item, Err> = Box<dyn Observer<Item, Err> + Send + 'stream>;

  type BoxedMutRefObserver<'stream, Item, Err> =
    Box<dyn for<'a> Observer<&'a mut Item, Err> + Send + 'stream>;

  fn subject<'stream, Item, Err>()
  -> Self::With<Subject<Self::BoxedObserver<'stream, Item, Err>, Self>> {
    Shared::create(Subject::default())
  }

  fn mut_ref_subject<'stream, Item, Err>()
  -> Self::With<Subject<Self::BoxedMutRefObserver<'stream, Item, Err>, Self>> {
    Shared::create(Subject::default())
  }
}

// ==========================================

// 6. Observer Implementations (特化)

// ==========================================

// Track A1: Local Boxed Observer (Item: Clone)

impl<Item, Err, C> Observer<Item, Err> for Subject<Box<dyn Observer<Item, Err>>, C>
where
  C: Context,
  Item: Clone,
  Err: Clone,
{
  fn next(&mut self, value: Item) {
    let mut guard = self.observers.rc_deref_mut();

    for (_, observer) in guard.list.iter_mut() {
      observer.next(value.clone());
    }
  }

  fn error(&mut self, e: Err) {
    let mut guard = self.observers.rc_deref_mut();

    let observers: SmallVec<[_; 2]> = guard.list.drain(..).collect();

    drop(guard);

    for (_, mut observer) in observers {
      observer.error(e.clone());
    }
  }

  fn complete(&mut self) {
    let mut guard = self.observers.rc_deref_mut();

    let observers: SmallVec<[_; 2]> = guard.list.drain(..).collect();

    drop(guard);

    for (_, mut observer) in observers {
      observer.complete();
    }
  }
}

// Track A2: Shared Boxed Observer (Item: Clone, Send)

impl<Item, Err, C> Observer<Item, Err> for Subject<Box<dyn Observer<Item, Err> + Send>, C>
where
  C: Context,
  Item: Clone,
  Err: Clone,
{
  fn next(&mut self, value: Item) {
    let mut guard = self.observers.rc_deref_mut();

    for (_, observer) in guard.list.iter_mut() {
      observer.next(value.clone());
    }
  }

  fn error(&mut self, e: Err) {
    let mut guard = self.observers.rc_deref_mut();

    let observers: SmallVec<[_; 2]> = guard.list.drain(..).collect();

    drop(guard);

    for (_, mut observer) in observers {
      observer.error(e.clone());
    }
  }

  fn complete(&mut self) {
    let mut guard = self.observers.rc_deref_mut();

    let observers: SmallVec<[_; 2]> = guard.list.drain(..).collect();

    drop(guard);

    for (_, mut observer) in observers {
      observer.complete();
    }
  }
}

// Track B: HRTB 可变引用

// ------------------------------------------------

// 专门针对 O 是 HRTB 类型的 Subject 实现 Observer<&mut T>

// 不需要 Clone，不需要 MutRef

impl<C, Err, T> Observer<&mut T, Err> for Subject<Box<dyn for<'a> Observer<&'a mut T, Err>>, C>
where
  C: Context,
  Err: Clone,
{
  fn next(&mut self, value: &mut T) {
    let mut guard = self.observers.rc_deref_mut();

    for (_, observer) in guard.list.iter_mut() {
      // Direct reborrow!

      observer.next(value);
    }
  }

  fn error(&mut self, e: Err) {
    let mut guard = self.observers.rc_deref_mut();

    let observers: SmallVec<[_; 2]> = guard.list.drain(..).collect();

    drop(guard);

    for (_, mut observer) in observers {
      observer.error(e.clone());
    }
  }

  fn complete(&mut self) {
    let mut guard = self.observers.rc_deref_mut();

    let observers: SmallVec<[_; 2]> = guard.list.drain(..).collect();

    drop(guard);

    for (_, mut observer) in observers {
      observer.complete();
    }
  }
}

// Track B2: Shared HRTB 可变引用 (+ Send)

impl<C, Err, T> Observer<&mut T, Err>
  for Subject<Box<dyn for<'a> Observer<&'a mut T, Err> + Send>, C>
where
  C: Context,
  Err: Clone,
{
  fn next(&mut self, value: &mut T) {
    let mut guard = self.observers.rc_deref_mut();

    for (_, observer) in guard.list.iter_mut() {
      // Direct reborrow!

      observer.next(value);
    }
  }

  fn error(&mut self, e: Err) {
    let mut guard = self.observers.rc_deref_mut();

    let observers: SmallVec<[_; 2]> = guard.list.drain(..).collect();

    drop(guard);

    for (_, mut observer) in observers {
      observer.error(e.clone());
    }
  }

  fn complete(&mut self) {
    let mut guard = self.observers.rc_deref_mut();

    let observers: SmallVec<[_; 2]> = guard.list.drain(..).collect();

    drop(guard);

    for (_, mut observer) in observers {
      observer.complete();
    }
  }
}

// ==========================================

// 7. Main

// ==========================================

fn main() {
  println!("=== Subject Factory POC (Final: Mut Factory + Lifetimes + CoreObservable) ===");

  println!("\n[Local Subject]");

  let subject = <Local<()> as ObservableFactory>::subject::<i32, String>();

  let mut subject_producer = subject.clone();

  // Test standard Subscribe

  let mut sub1 =
    (&*subject).subscribe(Box::new(PrintObserver("Local1")) as Box<dyn Observer<i32, String>>);

  subject_producer.next(10);

  sub1.unsubscribe();

  subject_producer.complete();

  // --- GAT CoreObservable Verification ---

  println!("\n[CoreObservable GAT Verification]");

  // Helper for owned items

  fn assert_core_observable<T, O, ItemGat>(_: T)
  where
    T: for<'a> CoreObservable<O, Item<'a> = ItemGat>,
  {
  }

  // Helper for HRTB items like `&'a mut Item`

  fn assert_core_observable_hrtb<T, O, Item>(_: T)
  where
    T: for<'a> CoreObservable<O, Item<'a> = &'a mut Item>,
  {
  }

  // 1. Local + Owned Item

  let local_subject = <Local<()> as ObservableFactory>::subject::<i32, String>();

  assert_core_observable::<_, Box<dyn Observer<i32, String>>, i32>((*local_subject).clone());

  println!("  ✅ Local + Owned Item: OK");

  // 2. Local + HRTB Item

  let local_mut_subject = <Local<()> as ObservableFactory>::mut_ref_subject::<i32, String>();

  assert_core_observable_hrtb::<_, Box<dyn for<'a> Observer<&'a mut i32, String>>, i32>(
    (*local_mut_subject).clone(),
  );

  println!("  ✅ Local + HRTB Item: OK");

  // 3. Shared + Owned Item

  let shared_subject = <Shared<()> as ObservableFactory>::subject::<i32, String>();

  assert_core_observable::<_, Box<dyn Observer<i32, String> + Send>, i32>(
    (*shared_subject).clone(),
  );

  println!("  ✅ Shared + Owned Item: OK");

  // 4. Shared + HRTB Item

  let shared_mut_subject = <Shared<()> as ObservableFactory>::mut_ref_subject::<i32, String>();

  assert_core_observable_hrtb::<_, Box<dyn for<'a> Observer<&'a mut i32, String> + Send>, i32>(
    (*shared_mut_subject).clone(),
  );

  println!("  ✅ Shared + HRTB Item: OK");

  // --- End Verification ---

  println!("\n[MutRef Subject (Via Factory)]");

  // 使用 Factory 创建支持 HRTB 的 Subject

  let mut_subject = <Local<()> as ObservableFactory>::mut_ref_subject::<i32, String>();

  let mut mut_producer = mut_subject.clone();

  // 订阅者 1：加 10

  (&*mut_subject)
    .subscribe(Box::new(MutObserver("A")) as Box<dyn for<'a> Observer<&'a mut i32, String>>);

  // 订阅者 2：乘 2

  (&*mut_subject)
    .subscribe(Box::new(MutObserver("B")) as Box<dyn for<'a> Observer<&'a mut i32, String>>);

  let mut value = 10;

  println!("  Value before: {}", value);

  // ✅ 此时直接传递 &mut value，不需要 MutRef 包装

  mut_producer.next(&mut value);

  println!("  Value after: {}", value);

  assert_eq!(value, 22); // (10 + 1) * 2 = 22

  println!("\n[Scoped Subject (Lifetime Support)]");

  {
    let x = 100;

    let subject = <Local<()> as ObservableFactory>::subject::<i32, ()>();

    let mut subject_producer = subject.clone();

    // 订阅捕获局部变量 x 的闭包

    (&*subject).subscribe(Box::new(ClosureObserver {
      cb: Box::new(move |v| println!("  Captured x={}, v={}", x, v)),
    }) as Box<dyn Observer<i32, ()>>);

    // 使用 producer 发送数据

    subject_producer.next(1);
  }
}

// --- Helpers ---

struct ClosureObserver<F> {
  cb: F,
}

impl<F: FnMut(i32)> Observer<i32, ()> for ClosureObserver<F> {
  fn next(&mut self, v: i32) {
    (self.cb)(v)
  }

  fn error(&mut self, _: ()) {}

  fn complete(&mut self) {}
}

struct PrintObserver(&'static str);

impl<T: std::fmt::Display, E> Observer<T, E> for PrintObserver {
  fn next(&mut self, v: T) {
    println!("  [{} next: {}", self.0, v);
  }

  fn error(&mut self, _: E) {}

  fn complete(&mut self) {
    println!("  [{}] complete", self.0);
  }
}

struct MutObserver(&'static str);

impl<'a> Observer<&'a mut i32, String> for MutObserver {
  fn next(&mut self, v: &'a mut i32) {
    if self.0 == "A" {
      *v += 1;
    } else {
      *v *= 2;
    }

    println!("  MutObserver[{}] saw: {}", self.0, *v);
  }

  fn error(&mut self, _: String) {}

  fn complete(&mut self) {}
}
