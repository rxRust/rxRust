use std::rc::Rc;
use std::cell::Cell;
use std::sync::{Arc, Mutex};

// ==================== 基础定义 ====================
pub trait Observer<Item, Err> {
    fn next(&mut self, v: Item);
}
impl<I, E> Observer<I, E> for () { fn next(&mut self, _: I) {} }
impl Observer<(), ()> for Rc<Cell<i32>> { fn next(&mut self, _: ()) {} }

pub trait Context: Sized {
    type Inner;
    fn into_inner(self) -> Self::Inner;
}

struct Local<T>(T);
impl<T> Context for Local<T> {
    type Inner = T;
    fn into_inner(self) -> T { self.0 }
}

struct Shared<T>(T);
impl<T> Context for Shared<T> {
    type Inner = T;
    fn into_inner(self) -> T { self.0 }
}

pub trait Subscription {}
impl Subscription for () {}

fn require_send<T: Send>(_: T) {}

// ==================== 关键：策略 Trait ====================

// 把订阅后的 "处理" 逻辑抽象出来
// 这不仅仅是 Check，而是 Act
pub trait SubscribeStrategy<O> {
    fn handle_observer(&self, observer: O);
}

// ==================== ObservableCore ====================

pub trait ObservableCore {
    type Item;
    type Err;
    type Unsub<C: Context>: Subscription;
    
    fn subscribe_core<C>(self, observer: C) -> Self::Unsub<C>
    where
        C: Context,
        C::Inner: Observer<Self::Item, Self::Err>,
        // 关键：Self 必须知道如何处理 C::Inner
        Self: SubscribeStrategy<C::Inner>; 
}

// ==================== 场景 1: MapOp (无约束) ====================

struct MapOp;

// MapOp 的处理逻辑：不做特殊约束
impl<O> SubscribeStrategy<O> for MapOp {
    fn handle_observer(&self, _observer: O) {
        println!("MapOp handling observer (no send required)");
    }
}

impl ObservableCore for MapOp {
    type Item = ();
    type Err = ();
    type Unsub<C: Context> = ();

    fn subscribe_core<C>(self, observer: C)
    where 
        C: Context, 
        C::Inner: Observer<(),()>, 
        Self: SubscribeStrategy<C::Inner>
    {
        // 委托给 Strategy
        self.handle_observer(observer.into_inner());
    }
}

// ==================== 场景 2: SharedSubject (Send 约束) ====================

struct SharedSubject {
    // 模拟内部存储，要求 Send
    observers: Arc<Mutex<Vec<Box<dyn Send>>>>, 
}

// 关键：只为 Send 的 O 实现 SubscribeStrategy
impl<O> SubscribeStrategy<O> for SharedSubject 
where O: Send + 'static
{
    fn handle_observer(&self, observer: O) {
        println!("SharedSubject handling observer (Send verified!)");
        
        // 在这里，编译器完全知道 O 是 Send 的！
        // 1. 可以调用 require_send
        require_send(observer); 
        
        // 2. 可以存入 Vec<Box<dyn Send>> (伪代码)
        // let boxed: Box<dyn Send> = Box::new(observer);
        // self.observers.lock().unwrap().push(boxed);
    }
}

impl ObservableCore for SharedSubject {
    type Item = ();
    type Err = ();
    type Unsub<C: Context> = ();

    fn subscribe_core<C>(self, observer: C)
    where 
        C: Context, 
        C::Inner: Observer<(),()>, 
        Self: SubscribeStrategy<C::Inner> 
    {
        // 同样是委托，代码一模一样！
        self.handle_observer(observer.into_inner());
    }
}

// ==================== Main ====================

fn main() {
    println!("Checking Strategy Pattern...");
    
    // 1. MapOp 接受 !Send
    let map = MapOp;
    let local = Local(Rc::new(Cell::new(0)));
    map.subscribe_core(local); 
    
    // 2. SharedSubject 应该拒绝 !Send
    let subject = SharedSubject { observers: Arc::new(Mutex::new(Vec::new())) };
    let local2 = Local(Rc::new(Cell::new(0)));
    
    // 下面这行如果取消注释，应该导致编译错误：
    // The trait bound `SharedSubject: SubscribeStrategy<Rc<Cell<i32>>>` is not satisfied.
    // subject.subscribe_core(local2);
    
    // 3. SharedSubject 应该接受 Send
    let subject2 = SharedSubject { observers: Arc::new(Mutex::new(Vec::new())) };
    let shared = Shared(());
    subject2.subscribe_core(shared);
}