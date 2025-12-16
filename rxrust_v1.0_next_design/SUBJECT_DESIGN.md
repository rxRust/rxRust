# Subject 1.0 最终架构设计

本文档归档了基于 POC 验证通过的最终 Subject 设计方案。该方案解决了旧版架构中的代码重复、循环引用风险，并统一了 Local 和 Shared 环境的实现模式。

## 1. 核心问题回顾

### 代码重复
旧版有 5 种 Subject 类型（`Subject`, `SubjectThreads`, 3 种 `MutRefSubject`），每种都有独立实现，维护困难且违反 DRY 原则。

### Chamber 双缓冲机制的废除
旧版使用 `chamber` 缓冲区来解决遍历时修改 `observers` 的问题，导致：
- 双倍内存开销
- 每次 emit 都要 `load()` 合并，性能差
- 代码复杂度高
- **无法解决循环引用问题**

### 循环引用风险
Subject 持有 Subscribers，Subscribers 持有闭包，闭包若捕获 Subject，会形成 `Subject -> Subscribers -> Closure -> Subject` 的强引用循环（尤其在 Local Rc 模式下）。

**解决方案**：rxRust 1.0 明确**不支持同步嵌套订阅**同一个 Subject。若需在回调中订阅，必须使用 `.delay(Duration::ZERO)` 将订阅操作调度到下一个任务循环，从而打破引用链。

---

## 2. 核心架构：基于 Context 的泛型实现

新架构的核心思想是**控制反转**：`Subject` 不再感知是 `Local` (Rc/RefCell) 还是 `Shared` (Arc/Mutex)，而是通过 `Context` trait 提供的抽象原语来管理内存和并发。

### 2.1 Context Trait 抽象

`Context` 定义了智能指针、内部可变性容器以及 Observer 的装箱策略。

```rust
// 伪代码，实际以 context 模块实现为准
pub trait Context {
    // 定义引用计数指针 (Rc 或 Arc)
    type Rc<T>: From<T> + Clone + RcDerefMut<Target = T>;
    
    // 定义创建时的包裹类型 (通常就是 Local<T> 或 Shared<T>)
    type With<T>: DerefMut<Target = T>;

    // 创建包裹类型的工厂方法
    fn create<T>(inner: T) -> Self::With<T>;
}
```

### 2.2 辅助 Trait：RcDerefMut & DynObserver

为了统一 `RefCell` 和 `Mutex` 的借用行为，引入 `RcDerefMut`：

```rust
pub trait RcDerefMut {
    type Target;
    type Guard<'a>: DerefMut<Target = Self::Target> where Self: 'a;
    fn rc_deref_mut(&self) -> Self::Guard<'_>;
}
```

为了实现 Object Safety（`Observer` trait 包含泛型方法或传值方法，不易直接做 Object），引入 `DynObserver`：

```rust
pub trait DynObserver<Item, Err> {
    fn box_next(&mut self, value: Item);
    fn box_error(self: Box<Self>, err: Err);
    fn box_complete(self: Box<Self>);
}
```

由于我们现在允许 `Subject` 的 `Observer` 捕获非 `'static` 生命周期，这意味着 `Subject::subscribe` 返回的 `Subscription` 虽然可能包含非 `'static` 的引用（取决于 `Observer`），但其本身是一个独立的结构体 `SubjectSubscription`。它持有内部状态的共享引用，并不借用 `Subject` 实例本身。

**生命周期与使用模式：**
虽然 `SubjectSubscription` 不直接借用 `Subject`，但为了代码的清晰性和避免潜在的逻辑混淆，我们推荐在复杂的订阅和发送场景中明确分离 **Consumer (订阅端)** 和 **Producer (发送端)** 的句柄，但这并非强制性的借用检查器要求。

```rust
// 示例:
let subject = Local::subject::<i32, String>();

// 订阅方
let subscription = subject.subscribe(|v| println!("Got {}", v));

// 发送方 (直接使用 subject 也是安全的，因为 subscription 不借用 subject)
let mut producer = subject.clone(); 
producer.next(10);
producer.complete();
```

### 2.3 Subject 结构体

`Subject` 变为依赖智能指针类型 `P` 的泛型结构体。它不再直接依赖 `Context`，而是依赖于 `P` (即 `Context::Rc`) 的具体实现。

```rust
pub struct Subject<P> {
    // P 是一个智能指针，指向 Subscribers<O>
    // 例如 Rc<RefCell<Subscribers<O>>> 或 Arc<Mutex<Subscribers<O>>>
    pub observers: P,
}
```

### 2.5 集成 CoreObservable

为了让 `Subject` 能够融入 rxRust 的操作符链条（如 `.map().filter()`），它必须实现 `CoreObservable` trait。

```rust
// 定义 Subject 专用的 Subscription
pub struct SubjectSubscription<P> {
    pub(crate) observers: Option<P>,
    pub(crate) id: usize,
}

impl<P, O> Subscription for SubjectSubscription<P>
where
    P: RcDerefMut<Target = Subscribers<O>> + Clone,
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

// 实现 CoreObservable
impl<Item, Err, P, O, SubO> CoreObservable<SubO> for Subject<P>
where
    P: RcDerefMut<Target = Subscribers<O>> + Clone,
    O: Observer<Item, Err> + ?Sized, // O 通常是 Box<dyn Observer<...>>
    SubO: Into<O>,
{
    type Item<'a> = Item;
    type Err = Err;
    type Unsub = SubjectSubscription<P>;

    fn subscribe(self, observer: SubO) -> Self::Unsub {
        // 直接调用 Subject 的 inherent 方法
        // 尽管 CoreObservable::subscribe 消耗 self，但 Subject 是轻量级句柄（持有 Rc），
        // 且 inherent subscribe 只需要 &self。
        Subject::subscribe(&self, observer)
    }
}
```

注意：`Subject::subscribe` 返回 `SubjectSubscription`。该结构体持有 `Subscribers` 的强引用 (`P`)，因此其生命周期不依赖于 `Subject` 实例本身，而是共享 `Subscribers` 的所有权。这使得 `Subject` 能够灵活地在不同的上下文中传递和使用。

## 3. 支持 &mut Item：HRTB 直接引用方案

Subject 支持 `Observer<&mut Item, Err>` 允许在订阅链中顺序修改数据。我们废弃了旧版繁琐的 `MutRef` 包装 struct，采用 **HRTB (Higher-Rank Trait Bounds)** 直接支持 `&mut T`。

### 3.1 核心思路

利用 Rust 的 HRTB 特性 (`for<'a>`)，让 `Subject` 能够持有可以处理任意生命周期引用的 `Observer`。

### 3.2 统一 Subject 结构体

不需要独立的 `MutRefSubject` 结构体，`Subject` 保持唯一且纯粹：

```rust
pub struct Subject<P> {
    pub observers: P,
}
```

### 3.3 分离 Observer 实现

利用 Rust Trait 系统的输入参数特化能力，为同一个 Subject 实现两种不同的 Observer：

**场景 A：普通广播 (Item: Clone)**

适用于 `Box<dyn Observer<Item, Err>>` 或 `Box<dyn Observer<Item, Err> + Send>`。

```rust
impl<P, O, Item, Err> Observer<Item, Err> for Subject<P>
where
    P: RcDerefMut<Target = Subscribers<O>>,
    O: Observer<Item, Err> + ?Sized, // O 实现了普通的 Observer
    Item: Clone,                     // 必须 Clone 以支持多播
{
    fn next(&mut self, value: Item) {
        let mut guard = self.observers.rc_deref_mut();
        // 标准广播逻辑：克隆分发
        for (_, observer) in guard.list.iter_mut() {
             observer.next(value.clone());
        }
    }
}
```

**场景 B：可变引用顺序传递 (HRTB &mut T)**

针对存储类型为 HRTB Observer 的 Subject，实现接受 `&mut T` 的 Observer。
这里针对 `Local` (非 Send) 和 `Shared` (Send) 场景分别实现了特化。

```rust
// 1. Local 场景 (O 不带 Send)
impl<P, Err, T> Observer<&mut T, Err> for Subject<P>
where
    P: RcDerefMut<Target = Subscribers<Box<dyn for<'a> Observer<&'a mut T, Err>>>>,
    Err: Clone,
{
    fn next(&mut self, value: &mut T) {
        let mut guard = self.observers.rc_deref_mut();
        for (_, observer) in guard.list.iter_mut() {
            // Safety: 通过 Reborrowing 机制，将同一个 &mut T 顺序借用给每个 Observer
            // 整个过程完全 Safe，依赖 Rust 借用检查器保证生命周期安全
            observer.next(value);
        }
    }
}

// 2. Shared 场景 (O 带 Send)
impl<P, Err, T> Observer<&mut T, Err> for Subject<P>
where
    P: RcDerefMut<Target = Subscribers<Box<dyn for<'a> Observer<&'a mut T, Err> + Send>>>,
    Err: Clone,
{
    fn next(&mut self, value: &mut T) {
        let mut guard = self.observers.rc_deref_mut();
        for (_, observer) in guard.list.iter_mut() {
            observer.next(value);
        }
    }
}
```

### 3.4 API 设计与使用示例

对于普通 `Clone` 类型，使用工厂方法创建。对于 HRTB 引用类型，需显式指定类型（或通过辅助别名）。

```rust
// === 1. 普通模式 (Item: Clone) ===
let subject = Local::subject::<i32, String>();
subject.subscribe(|v| println!("Got {}", v));
subject.next(10); // 广播 10


// === 2. 可变引用模式 (HRTB) ===
// 定义 HRTB Observer 类型
type HrtbObserver = Box<dyn for<'a> Observer<&'a mut i32, String>>;

// 创建 Subject (Local)
let mut mut_subject = Local::create(Subject::<Local::Rc<Subscribers<HrtbObserver>>>::default());

// 订阅 (直接接收 &mut i32)
mut_subject.subscribe(|v: &mut i32| {
    *v += 10;
});
mut_subject.subscribe(|v: &mut i32| {
    *v *= 2;
});

// 发送数据
let mut value = 10;
mut_subject.next(&mut value); 
// value 变为 (10 + 10) * 2 = 40
```

### 3.5 方案优势

1.  **架构极简**：只有一个 `Subject` 结构体。
2.  **零成本抽象**：移除了 `MutRef` 包装结构体，直接传递原生 `&mut T`。
3.  **支持多线程**：新增了对 `Box<dyn ... + Send>` 的 HRTB 支持，使得 `Shared` 环境下也能安全使用可变引用流。
4.  **符合直觉**：API 直接操作 `&mut T`，无需中间层。
5.  **解耦 Context**：`Subject` 不再直接依赖 `Context` trait，而是通过泛型 `P` 与具体的智能指针实现解耦，使 `Subject` 结构体更纯粹。

---

## 4. 组件扩展：BehaviorSubject

基于同样的模式，我们定义 `BehaviorSubject`。

### 4.1 结构设计

```rust
pub struct BehaviorSubject<P, VP> {
    pub subject: Subject<P>,
    pub value: VP, // 存储当前值的智能指针 (如 Rc<RefCell<Item>>)
}
```

### 4.2 Behavior Trait

提供对当前值的访问和修改能力。

```rust
pub trait Behavior {
    type Item;
    
    fn peek(&self) -> Self::Item;
    fn next_by<F>(&mut self, f: F) where F: FnOnce(&Self::Item) -> Self::Item;
}

impl<P, VP, O, Item, Err> Behavior for BehaviorSubject<P, VP>
where
    P: RcDerefMut<Target = Subscribers<O>> + Clone,
    VP: RcDerefMut<Target = Item>,
    O: Observer<Item, Err> + ?Sized,
    Item: Clone,
    Err: Clone,
{
    type Item = Item;
    
    fn peek(&self) -> Item {
        self.value.rc_deref().clone()
    }
    
    fn next_by<F>(&mut self, f: F) 
    where F: FnOnce(&Item) -> Item 
    {
        let new_value = {
            let current = self.value.rc_deref();
            f(&*current)
        };
        self.next(new_value);
    }
}
```

### 3.3 Observer 实现

当 BehaviorSubject 接收到新值时，更新内部存储并转发给 Subject。

```rust
impl<P, VP, O, Item, Err> Observer<Item, Err> for BehaviorSubject<P, VP>
where
    P: RcDerefMut<Target = Subscribers<O>>,
    VP: RcDerefMut<Target = Item>,
    O: Observer<Item, Err> + ?Sized,
    Item: Clone,
    Err: Clone,
{
    fn next(&mut self, value: Item) {
        *self.value.rc_deref_mut() = value.clone();
        self.subject.next(value);
    }
    // ...
}
```

---

## 5. Factory 与 API 设计

通过 `ObservableFactory` trait 和关联类型别名简化创建流程。

### 5.1 ObservableFactory

利用 GAT 定义上下文默认的 `BoxedObserver` 和 `BoxedMutRefObserver` 类型，允许它们捕获非 `'static` 生命周期。同时，工厂方法强制返回统一的 `Subject` 结构体（被 `Context::With` 包裹），且自身也带生命周期参数。

```rust
pub trait ObservableFactory: Context {
    // 普通 Observer (Item: Clone)，允许捕获 'a 生命周期
    type BoxedObserver<'a, Item, Err>;
    
    // 引用可变 Observer (Item: &mut T) - HRTB，允许捕获 'a 生命周期
    type BoxedMutRefObserver<'a, Item, Err>;

    // 普通 Subject 创建
    // Subject<P>，其中 P = Self::Rc<Subscribers<Self::BoxedObserver<'a, Item, Err>>>
    fn subject<'a, Item, Err>() -> Self::With<Subject<Self::Rc<Subscribers<Self::BoxedObserver<'a, Item, Err>>>>>
    where
        Self: Sized;

    // 引用可变 Subject 创建
    fn mut_ref_subject<'a, Item, Err>() -> Self::With<Subject<Self::Rc<Subscribers<Self::BoxedMutRefObserver<'a, Item, Err>>>>>
    where
        Self: Sized;
}

// Local 实现示例
impl ObservableFactory for Local<()> {
    type BoxedObserver<'a, Item, Err> = Box<dyn Observer<Item, Err> + 'a>;
    type BoxedMutRefObserver<'a, Item, Err> = Box<dyn for<'b> Observer<&'b mut Item, Err> + 'a>;

    fn subject<'a, Item, Err>() -> Self::With<Subject<Self::Rc<Subscribers<Self::BoxedObserver<'a, Item, Err>>>>> {
        Local::create(Subject { observers: Default::default() })
    }
    
    fn mut_ref_subject<'a, Item, Err>() -> Self::With<Subject<Self::Rc<Subscribers<Self::BoxedMutRefObserver<'a, Item, Err>>>>> {
        Local::create(Subject { observers: Default::default() })
    }
}

// Shared 依然可以强制要求 Send + 'static 如果需要，或者支持 scoped threads ('a + Send)
impl ObservableFactory for Shared<()> {
    type BoxedObserver<'a, Item, Err> = Box<dyn Observer<Item, Err> + Send + 'a>;
    type BoxedMutRefObserver<'a, Item, Err> = Box<dyn for<'b> Observer<&'b mut Item, Err> + Send + 'a>;

    fn subject<'a, Item, Err>() -> Self::With<Subject<Self::Rc<Subscribers<Self::BoxedObserver<'a, Item, Err>>>>> {
        Shared::create(Subject { observers: Default::default() })
    }

    fn mut_ref_subject<'a, Item, Err>() -> Self::With<Subject<Self::Rc<Subscribers<Self::BoxedMutRefObserver<'a, Item, Err>>>>> {
        Shared::create(Subject { observers: Default::default() })
    }
}
```

### 5.2 使用示例

API 保持简洁一致，支持快捷创建方法。同时，需要注意在同一作用域内订阅和发送数据时的**Producer-Consumer Clone 模式**。

```rust
// === 1. 普通模式 (Local) ===
let subject = Local::subject::<i32, String>();
let mut producer = subject.clone(); // 克隆一个 producer 句柄用于发送数据

// 订阅方使用原始 subject
subject.subscribe(|v| println!("Local: {}", v));

// 发送方使用 producer 句柄
producer.next(10);
producer.complete();


// === 2. 引用可变模式 (Local) ===
let mut_subject = Local::mut_ref_subject::<i32, String>();
let mut mut_producer = mut_subject.clone(); // 克隆一个 producer 句柄

mut_subject.subscribe(|v: &mut i32| *v += 1);
mut_subject.subscribe(|v: &mut i32| *v *= 2);

let mut value = 10;
mut_producer.next(&mut value); // 使用 producer 发送
// value: (10 + 1) * 2 = 22


// === 3. Scoped Subject (捕获局部变量) ===
// 这种场景下，subject 的生命周期与被捕获的局部变量绑定。
{
    let x = 100;
    let subject = Local::subject::<i32, ()>();
    let mut producer = subject.clone(); // 克隆 producer 句柄

    // 订阅一个捕获局部变量 x 的闭包
    subject.subscribe(|v| println!("Captured x={}, v={}", x, v));

    producer.next(1); // 使用 producer 发送
} // x 和 subject 在这里超出作用域，被正确 drop


// === 4. Shared 模式 (支持跨线程) ===
let shared_subject = Shared::subject::<i32, String>();
let mut shared_producer = shared_subject.clone(); // 克隆 producer
shared_subject.subscribe(|v| println!("Shared: {}", v));
shared_producer.next(20);

// let shared_mut = Shared::mut_ref_subject::<i32, String>(); // 同样支持
```

---

## 6. 收益总结

1.  **架构统一**：一套代码同时支持 `Local` 和 `Shared`，无需宏或重复代码。
2.  **类型安全与灵活**：不再强依赖 `Context` 定义 Observer 类型，而是通过泛型 `O` 或 Factory GAT 灵活指定。对于 Shared 环境，Factory 可自动提供 `Send` 的 Observer，无需硬编码。
3.  **性能优化**：
    - 移除 `chamber` 双缓冲，减少内存分配和拷贝。
    - 使用 `SmallVec` 优化少量订阅者的场景。
4.  **&mut 支持**：通过 HRTB (`for<'a> Observer<&'a mut T>`) 直接支持可变引用的顺序传递，无需 `MutRef` 包装，零冲突，且支持 `Send` 环境。
5.  **扩展性**：新增 Context 类型（如支持更多种类的调度策略或指针）无需修改 Subject 代码。