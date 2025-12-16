pub mod core {
    // 定义在同一个模块内，避免 import 问题
    pub trait Context {
        type Inner;
    }

    pub trait Observer<Item, Err> {
        fn next(&mut self, v: Item);
    }

    pub trait CoreObservable<O> {
        type Item;
        type Err;
        fn subscribe(self, observer: O);
    }

    // ==========================================
    // 关键点：这里的可见性
    // ==========================================
    
    // 如果是 private (mod hidden)，编译器会报错 E0446
    // 如果是 pub(crate)，编译器允许，因为这是库内部可见
    pub(crate) mod hidden {
        // use super::Observer; // 这里的 super 引用问题先忽略，直接重定义简化的 observer 模拟
        pub struct DiscardingObserver;
        
        // 模拟实现 Observer
        impl<Item, Err> super::Observer<Item, Err> for DiscardingObserver {
             fn next(&mut self, _v: Item) {}
        }
    }

    pub trait Observable: Context {
        type Item;
        type Err;
    }

    // BLANKET IMPL
    // 这里使用了 pub(crate) 的类型
    impl<T> Observable for T
    where
        T: Context,
        T::Inner: CoreObservable<hidden::DiscardingObserver>, 
    {
        type Item = <T::Inner as CoreObservable<hidden::DiscardingObserver>>::Item;
        type Err = <T::Inner as CoreObservable<hidden::DiscardingObserver>>::Err;
    }
}