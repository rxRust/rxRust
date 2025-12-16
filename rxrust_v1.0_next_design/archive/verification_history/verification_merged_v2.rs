use std::marker::PhantomData;
use std::rc::Rc;

// ==================== Basic Infrastructure ====================

pub trait Subscription {
    fn unsubscribe(self);
}
impl Subscription for () {
    fn unsubscribe(self) {}
}

pub trait Observer<Item, Err> {
    fn next(&mut self, value: Item);
    fn error(self, err: Err);
    fn complete(self);
}

// ==================== Scheduler System ====================

pub trait Runnable {
    fn run(self);
}

pub struct OnNext<O, Item, Err> {
    pub observer: O,
    pub item: Item,
    pub _p: PhantomData<Err>,
}
impl<O, Item, Err> Runnable for OnNext<O, Item, Err>
where
    O: Observer<Item, Err>,
{
    fn run(mut self) {
        self.observer.next(self.item);
    }
}

pub struct OnError<O, Item, Err> {
    pub observer: O,
    pub err: Err,
    pub _p: PhantomData<Item>,
}
impl<O, Item, Err> Runnable for OnError<O, Item, Err>
where
    O: Observer<Item, Err>,
{
    fn run(self) {
        self.observer.error(self.err);
    }
}

pub struct OnComplete<O, Item, Err> {
    pub observer: O,
    pub _p: PhantomData<(Item, Err)>,
}
impl<O, Item, Err> Runnable for OnComplete<O, Item, Err>
where
    O: Observer<Item, Err>,
{
    fn run(self) {
        self.observer.complete();
    }
}

pub trait Scheduler<F> {
    fn schedule(&self, task: F);
}

#[derive(Clone)]
pub struct LocalScheduler;
impl<F> Scheduler<F> for LocalScheduler
where
    F: Runnable,
{
    fn schedule(&self, task: F) {
        task.run();
    }
}

#[derive(Clone)]
pub struct SharedScheduler;
impl<F> Scheduler<F> for SharedScheduler
where
    F: Runnable + Send,
{
    fn schedule(&self, task: F) {
        let _ = task;
    }
}

// ==================== Merged CoreObservable ====================

pub trait CoreObservable<O> {
    // We remove Item/Err from here. They are implicit in the relationship between O and Self.
    type Unsub: Subscription;
    fn subscribe(self, observer: O) -> Self::Unsub;
}

// ==================== Implementation: ObserveOn ====================

pub struct ObserveOn<S, Sched> {
    pub source: S,
    pub scheduler: Sched,
}

// Removed Item/Err from the struct!
pub struct ObserveOnObserver<O, Sched> {
    pub observer: O,
    pub scheduler: Sched,
}

// 1. Implement Observer for the Wrapper
// Note: We use generic Item/Err here.
impl<O, Sched, Item, Err> Observer<Item, Err>
    for ObserveOnObserver<O, Sched>
where
    O: Observer<Item, Err> + Clone,
    Sched: Scheduler<OnNext<O, Item, Err>>
        + Scheduler<OnError<O, Item, Err>>
        + Scheduler<OnComplete<O, Item, Err>>,
{
    fn next(&mut self, value: Item) {
        let task = OnNext {
            observer: self.observer.clone(),
            item: value,
            _p: PhantomData,
        };
        self.scheduler.schedule(task);
    }
    fn error(self, err: Err) {
        let task = OnError {
            observer: self.observer.clone(),
            err,
            _p: PhantomData,
        };
        self.scheduler.schedule(task);
    }
    fn complete(self) {
        let task = OnComplete {
            observer: self.observer.clone(),
            _p: PhantomData,
        };
        self.scheduler.schedule(task);
    }
}

// 2. Implement CoreObservable for ObserveOn
impl<S, Sched, O> CoreObservable<O> for ObserveOn<S, Sched>
where
    Sched: Clone,
    // S must accept our wrapper. 
    // Note: We don't explicitly mention Item/Err here! 
    // Rust should infer that if S accepts ObserveOnObserver<O, Sched>, 
    // then ObserveOnObserver<O, Sched> must be a valid Observer for S's items.
    S: CoreObservable<ObserveOnObserver<O, Sched>>,
{
    type Unsub = <S as CoreObservable<ObserveOnObserver<O, Sched>>>::Unsub;

    fn subscribe(self, observer: O) -> Self::Unsub {
        let wrapped = ObserveOnObserver {
            observer,
            scheduler: self.scheduler,
        };
        self.source.subscribe(wrapped)
    }
}

// ==================== Mock Source ====================

pub struct MockSource;

impl<O> CoreObservable<O> for MockSource
where
    O: Observer<i32, ()>,
{
    type Unsub = ();

    fn subscribe(self, _observer: O) -> () {}
}

// ==================== Validation ====================

#[derive(Clone)]
struct RcObserver(Rc<i32>); 
impl Observer<i32, ()> for RcObserver {
    fn next(&mut self, _: i32) {}
    fn error(self, _: ()) {}
    fn complete(self) {}
}

#[derive(Clone)]
struct SendObserver; 
impl Observer<i32, ()> for SendObserver {
    fn next(&mut self, _: i32) {}
    fn error(self, _: ()) {}
    fn complete(self) {}
}

fn main() {
    // 1. Local + Local
    {
        let op = ObserveOn {
            source: MockSource,
            scheduler: LocalScheduler,
        };
        let obs = RcObserver(Rc::new(1));
        op.subscribe(obs);
    }

    // 2. Shared + Shared
    {
        let op = ObserveOn {
            source: MockSource,
            scheduler: SharedScheduler,
        };
        let obs = SendObserver;
        op.subscribe(obs);
    }
    
    // 3. Local + Shared (Should fail)
    {
        let op = ObserveOn {
            source: MockSource,
            scheduler: SharedScheduler,
        };
        let obs = RcObserver(Rc::new(1));
        // op.subscribe(obs); 
    }
}
