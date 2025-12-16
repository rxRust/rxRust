use std::time::Duration;
use std::rc::Rc;

pub trait Subscription {}
impl Subscription for () {}

// ==================== Scheduler ====================
pub trait Scheduler<F> {
    fn schedule(&self, task: F, _delay: Duration) -> impl Subscription;
}

pub struct LocalScheduler;
impl<F> Scheduler<F> for LocalScheduler 
where F: FnOnce() + 'static 
{
    fn schedule(&self, task: F, _delay: Duration) -> impl Subscription { task(); () }
}

pub struct SharedScheduler;
impl<F> Scheduler<F> for SharedScheduler 
where F: FnOnce() + Send + 'static 
{
    fn schedule(&self, task: F, _delay: Duration) -> impl Subscription { task(); () }
}

// ==================== Context Scheduler Trait ====================

// 这是一个 helper trait，用来根据 Context 的类型 (Local/Shared)
// 为 Action F 施加不同的约束 (无 / Send)。
pub trait ContextScheduler<Action> {
    fn schedule(self, delay: Duration, action: Action) -> impl Subscription;
}

// ==================== Context ====================

pub trait Context: Sized {
    type Inner;
    
    // 我们在这里无法直接写 `type ActionBound` 因为它是针对 F 的。
    // 所以我们直接用方法泛型，并依赖 Self 的实现来做 check。
    
    fn schedule_action<F>(self, delay: Duration, action: F) -> impl Subscription
    where 
        Self: ContextScheduler<F>; // 委托给 ContextScheduler 检查
}

// --- Local 实现 ---
struct Local<T>(T);

// Local 允许 F 是 !Send
impl<T, F> ContextScheduler<F> for Local<T>
where 
    T: 'static,
    F: FnOnce(T) + 'static // No Send required!
{
    fn schedule(self, delay: Duration, action: F) -> impl Subscription {
        let inner = self.0;
        let task = move || action(inner);
        LocalScheduler.schedule(task, delay)
    }
}

impl<T> Context for Local<T> 
where T: 'static 
{
    type Inner = T;
    fn schedule_action<F>(self, delay: Duration, action: F) -> impl Subscription
    where Self: ContextScheduler<F> 
    {
        <Self as ContextScheduler<F>>::schedule(self, delay, action)
    }
}

// --- Shared 实现 ---
struct Shared<T>(T);

// Shared 强制 F 是 Send
impl<T, F> ContextScheduler<F> for Shared<T>
where 
    T: Send + 'static,
    F: FnOnce(T) + Send + 'static // Send required!
{
    fn schedule(self, delay: Duration, action: F) -> impl Subscription {
        let inner = self.0;
        let task = move || action(inner);
        SharedScheduler.schedule(task, delay)
    }
}

impl<T> Context for Shared<T> 
where T: Send + 'static 
{
    type Inner = T;
    fn schedule_action<F>(self, delay: Duration, action: F) -> impl Subscription
    where Self: ContextScheduler<F> 
    {
        <Self as ContextScheduler<F>>::schedule(self, delay, action)
    }
}

// ==================== Main 验证 ====================

fn main() {
    println!("Testing flexible constraints (v2 fixed)...");

    // 1. Local + !Send Action
    let local_ctx = Local(());
    let rc = Rc::new(100); 
    let action_not_send = move |_| {
        println!("Accessing Rc in Local: {}", rc);
    };
    // 成功：Local 接受 !Send action
    local_ctx.schedule_action(Duration::from_secs(0), action_not_send);

    // 2. Shared + Send Action
    let shared_ctx = Shared(());
    let action_send = move |_| { println!("Accessing Send action in Shared"); };
    // 成功：Shared 接受 Send action
    shared_ctx.schedule_action(Duration::from_secs(0), action_send);
}