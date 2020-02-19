use crate::observer::{
    observer_complete_proxy_impl, observer_error_proxy_impl,
};
use crate::ops::SharedOp;
use crate::prelude::*;
/// Ignore the first `count` values emitted by the source Observable.
///
/// `skip` returns an Observable that ignore the first `count` values
/// emitted by the source Observable. If the source emits fewer than `count`
/// values then 0 of its values are emitted. After that, it completes,
/// regardless if the source completes.
///
/// # Example
/// Ignore the first 5 seconds of an infinite 1-second interval Observable
///
/// ```
/// # use rxrust::{
///   ops::{Skip}, prelude::*,
/// };
///
/// observable::from_iter(0..10).skip(5).subscribe(|v| println!("{}", v));
///

/// // print logs:
/// // 6
/// // 7
/// // 8
/// // 9
/// // 10
/// ```
///
pub trait Skip {
    fn skip(self, count: u32) -> SkipOp<Self>
        where
            Self: Sized,
    {
        SkipOp {
            source: self,
            count,
        }
    }
}

impl<O> Skip for O {}

pub struct SkipOp<S> {
    source: S,
    count: u32,
}

impl<S> IntoShared for SkipOp<S>
    where
        S: IntoShared,
{
    type Shared = SharedOp<SkipOp<S::Shared>>;
    fn to_shared(self) -> Self::Shared {
        SharedOp(SkipOp {
            source: self.source.to_shared(),
            count: self.count,
        })
    }
}

impl<O, U, S> RawSubscribable<Subscriber<O, U>> for SkipOp<S>
    where
        S: RawSubscribable<Subscriber<SkipObserver<O, U>, U>>,
        U: SubscriptionLike + Clone + 'static,
{
    type Unsub = S::Unsub;
    fn raw_subscribe(self, subscriber: Subscriber<O, U>) -> Self::Unsub {
        let subscriber = Subscriber {
            observer: SkipObserver {
                observer: subscriber.observer,
                subscription: subscriber.subscription.clone(),
                count: self.count,
                hits: 0,
            },
            subscription: subscriber.subscription,
        };
        self.source.raw_subscribe(subscriber)
    }
}

pub struct SkipObserver<O, S> {
    observer: O,
    subscription: S,
    count: u32,
    hits: u32,
}

impl<S, ST> IntoShared for SkipObserver<S, ST>
    where
        S: IntoShared,
        ST: IntoShared,
{
    type Shared = SkipObserver<S::Shared, ST::Shared>;
    fn to_shared(self) -> Self::Shared {
        SkipObserver {
            observer: self.observer.to_shared(),
            subscription: self.subscription.to_shared(),
            count: self.count,
            hits: self.hits,
        }
    }
}

impl<Item, O, U> ObserverNext<Item> for SkipObserver<O, U>
    where
        O: ObserverNext<Item> + ObserverComplete,
        U: SubscriptionLike,
{
    fn next(&mut self, value: Item) {
        self.hits += 1;
        if self.hits > self.count {
            self.observer.next(value);
            if self.hits == self.count {
                self.complete();
                self.subscription.unsubscribe();
            }
        }
    }
}

observer_error_proxy_impl!(SkipObserver<O, U>, O, observer, <O,U>);
observer_complete_proxy_impl!(SkipObserver<O, U>, O,  observer, <O,U>);

impl<S> Fork for SkipOp<S>
    where
        S: Fork,
{
    type Output = SkipOp<S::Output>;
    fn fork(&self) -> Self::Output {
        SkipOp {
            source: self.source.fork(),
            count: self.count,
        }
    }
}

#[cfg(test)]
mod test {
    use super::Skip;
    use crate::prelude::*;

    #[test]
    fn base_function() {
        let mut completed = false;
        let mut next_count = 0;

        observable::from_iter(0..100)
            .skip(5)
            .subscribe_complete(|_| next_count += 1, || completed = true);

        assert_eq!(next_count, 95);
        assert_eq!(completed, true);
    }

    #[test]
    fn base_empty_function() {
        let mut completed = false;
        let mut next_count = 0;

        observable::from_iter(0..100)
            .skip(101)
            .subscribe_complete(|_| next_count += 1, || completed = true);

        assert_eq!(next_count, 0);
        assert_eq!(completed, true);
    }

    #[test]
    fn skip_support_fork() {
        let mut nc1 = 0;
        let mut nc2 = 0;
        {
            let skip5 = observable::from_iter(0..100).skip(5);
            let f1 = skip5.fork();
            let f2 = skip5.fork();

            f1.skip(5).fork().subscribe(|_| nc1 += 1);
            f2.skip(5).fork().subscribe(|_| nc2 += 1);
        }
        assert_eq!(nc1, 90);
        assert_eq!(nc2, 90);
    }

    #[test]
    fn into_shared() {
        observable::from_iter(0..100)
            .skip(5)
            .skip(5)
            .to_shared()
            .subscribe(|_| {});
    }
}
