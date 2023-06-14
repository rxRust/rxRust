use crate::{
  prelude::*,
  rc::{MutArc, MutRc, RcDeref, RcDerefMut},
};
use std::collections::VecDeque;

pub struct MergeAllOp<'a, S, ObservableItem> {
  pub concurrent: usize,
  pub source: S,
  _marker: TypeHint<&'a ObservableItem>,
}

pub struct MergeAllOpThreads<S, ObservableItem> {
  pub source: S,
  pub concurrent: usize,
  _marker: TypeHint<ObservableItem>,
}

macro_rules! impl_new_method {
  ($name: ident $(,$lf:lifetime)?) => {
    impl<$($lf,)? S, ObservableItem> $name<$($lf,)? S, ObservableItem> {
      #[inline]
      pub(crate) fn new(source: S, concurrent: usize) -> Self {
        Self {
          concurrent,
          source,
          _marker: TypeHint::default(),
        }
      }
    }
  };
}

impl_new_method!(MergeAllOp, 'a);
impl_new_method!(MergeAllOpThreads);

macro_rules! impl_observable_method {
  ($subscription: ty, $box_unsub: ty, $outside_observer: ident, $rc: ident) => {
    type Unsub = $subscription;

    fn actual_subscribe(self, observer: O) -> Self::Unsub {
      let mut subscription = Self::Unsub::default();

      let observer_data = ObserverData {
        observer,
        subscribe_tasks: <_>::default(),
        outside_completed: false,
        subscribed: 0,
        concurrent: self.concurrent,
      };
      let observer_data = $rc::own(Some(observer_data));
      let merge_all_observer = $outside_observer {
        observer_data,
        subscription: subscription.clone(),
        _hint: TypeHint::new(),
      };
      let unsub = self.source.actual_subscribe(merge_all_observer);
      subscription.append(<$box_unsub>::new(unsub));
      subscription
    }
  };
}

impl<'a, ObservableItem, Item, Err, O, S> Observable<Item, Err, O>
  for MergeAllOp<'a, S, ObservableItem>
where
  O: Observer<Item, Err> + 'a,
  S: Observable<ObservableItem, Err, OutsideObserver<'a, O, Item>>,
  ObservableItem: Observable<Item, Err, InnerObserver<'a, O>> + 'a,
  S::Unsub: 'a,
  ObservableItem::Unsub: 'a,
{
  impl_observable_method!(
    MultiSubscription<'a>,
    BoxSubscription<'a>,
    OutsideObserver,
    MutRc
  );
}

impl<'a, ObservableItem, Item, Err, S> ObservableExt<Item, Err>
  for MergeAllOp<'a, S, ObservableItem>
where
  S: ObservableExt<ObservableItem, Err>,
  ObservableItem: ObservableExt<Item, Err>,
{
}

impl<ObservableItem, Item, Err, O, S> Observable<Item, Err, O>
  for MergeAllOpThreads<S, ObservableItem>
where
  O: Observer<Item, Err> + Send + 'static,
  S: Observable<ObservableItem, Err, OutsideObserverThreads<O, Item>>,
  ObservableItem:
    Observable<Item, Err, InnerObserverThreads<O>> + Send + 'static,
  S::Unsub: Send + 'static,
  ObservableItem::Unsub: Send + 'static,
{
  impl_observable_method!(
    MultiSubscriptionThreads,
    BoxSubscriptionThreads,
    OutsideObserverThreads,
    MutArc
  );
}

impl<ObservableItem, Item, Err, S> ObservableExt<Item, Err>
  for MergeAllOpThreads<S, ObservableItem>
where
  S: ObservableExt<ObservableItem, Err>,
  ObservableItem: ObservableExt<Item, Err>,
{
}

pub struct OutsideObserver<'a, O, Item> {
  observer_data: MutRc<Option<ObserverDataLocal<'a, O>>>,
  subscription: MultiSubscription<'a>,
  _hint: TypeHint<Item>,
}

pub struct OutsideObserverThreads<O, Item> {
  observer_data: MutArc<Option<ObserverDataThreads<O>>>,
  subscription: MultiSubscriptionThreads,
  _hint: TypeHint<Item>,
}

struct ObserverData<O, Lazy> {
  observer: O,
  subscribe_tasks: VecDeque<Lazy>,
  outside_completed: bool,
  subscribed: usize,
  concurrent: usize,
}

type ObserverDataLocal<'a, O> = ObserverData<O, Box<dyn FnOnce() + 'a>>;
struct InnerObserver<'a, O>(MutRc<Option<ObserverDataLocal<'a, O>>>);

type ObserverDataThreads<O> = ObserverData<O, Box<dyn FnOnce() + Send>>;
struct InnerObserverThreads<O>(MutArc<Option<ObserverDataThreads<O>>>);

macro_rules! impl_inner_observer {
  ($ty:ty $(, $lf:lifetime)?) => {
    impl<$($lf,)? Item, Err, O> Observer<Item, Err> for $ty
    where
      O: Observer<Item, Err>,
    {
      fn next(&mut self, value: Item) {
        if let Some(data) = self.0.rc_deref_mut().as_mut() {
          data.observer.next(value)
        }
      }

      fn error(self, err: Err) {
        if let Some(data) = self.0.rc_deref_mut().take() {
          data.observer.error(err)
        }
      }

      fn complete(self) {
        let mut inner = self.0.rc_deref_mut();
        if let Some(data) = inner.as_mut() {
          if let Some(task) = data.subscribe_tasks.pop_front() {
            task();
          } else {
            data.subscribed -= 1;
            if data.subscribed == 0 && data.outside_completed {
              inner.take().unwrap().observer.complete();
            }
          }
        }
      }

      fn is_finished(&self) -> bool {
        self.0.rc_deref().as_ref().map_or(true, |data| {
          data.observer.is_finished()
        })
      }
    }
  };
}

impl_inner_observer!(InnerObserver<'a, O>, 'a);
impl_inner_observer!(InnerObserverThreads<O>);

macro_rules! impl_outside_observer {
  ($outside_ty: ty, $inner_ty: ty, $box_unsub: ty, $($lf:lifetime)? $($send:ident)?) => {
    impl<$($lf,)? Item, Err, O, ObservableItem> Observer<ObservableItem, Err>
      for $outside_ty
    where
      O: Observer<Item, Err> $(+ $lf)? $(+ $send + 'static)?,
      ObservableItem: Observable<Item, Err, $inner_ty> $(+ $lf)? $(+ $send + 'static)?,
      ObservableItem::Unsub: $($lf)? $($send + 'static)?,
    {
      fn next(&mut self, value: ObservableItem) {
        let mut observer_data = self.observer_data.rc_deref_mut();
        if let Some(data) = observer_data.as_mut() {
          if data.subscribed < data.concurrent {
            data.subscribed += 1;
            drop(observer_data);
            let unsub = value
              .actual_subscribe(<$inner_ty>::new(self.observer_data.clone()));
            let box_unsub = <$box_unsub>::new(unsub);
            self.subscription.append(box_unsub);
          } else {
            let observer_data = self.observer_data.clone();
            let mut subscription = self.subscription.clone();
            data.subscribe_tasks.push_back(Box::new(move || {
              let unsub = value.actual_subscribe(<$inner_ty>::new(observer_data));
              let box_unsub = <$box_unsub>::new(unsub);
              subscription.append(box_unsub);
            }));
          }
        }
      }

      fn error(self, err: Err) {
        if let Some(data) = self.observer_data.rc_deref_mut().take() {
          data.observer.error(err);
        }
      }

      fn complete(self) {
        let mut data = self.observer_data.rc_deref_mut();
        if let Some(inner) = data.as_mut() {
          inner.outside_completed = true;
          if inner.subscribed == 0 && inner.subscribe_tasks.is_empty() {
            data.take().unwrap().observer.complete();
          }
        }
      }

      fn is_finished(&self) -> bool {
        self.observer_data.rc_deref().as_ref().map_or(true, |data| {
          data.observer.is_finished()
        })
      }
    }
  };
}

impl_outside_observer!(OutsideObserver<'a, O, Item>, InnerObserver<'a, O>, BoxSubscription<'a>, 'a);
impl_outside_observer!(OutsideObserverThreads<O, Item>, InnerObserverThreads<O>, BoxSubscriptionThreads, Send);

impl<'a, O> InnerObserver<'a, O> {
  fn new(data: MutRc<Option<ObserverDataLocal<'a, O>>>) -> Self {
    InnerObserver(data)
  }
}

impl<O> InnerObserverThreads<O> {
  fn new(data: MutArc<Option<ObserverDataThreads<O>>>) -> Self {
    InnerObserverThreads(data)
  }
}
#[cfg(test)]
mod test {
  use crate::observable::fake_timer::FakeClock;
  use futures::executor::LocalPool;

  use super::*;
  use std::{cell::RefCell, rc::Rc, time::Duration};

  #[test]
  fn smoke() {
    let values = Rc::new(RefCell::new(vec![]));
    let c_values = values.clone();
    let clock = FakeClock::default();

    observable::from_iter(
      (0..3).map(|_| clock.interval(Duration::from_millis(1)).take(5)),
    )
    .merge_all(2)
    .subscribe(move |i| values.borrow_mut().push(i));
    clock.advance(Duration::from_millis(11));

    assert_eq!(
      &*c_values.borrow(),
      &[0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 0, 1, 2, 3, 4]
    );
  }

  #[test]
  fn fix_inner_unsubscribe() {
    let mut values = vec![];
    let c_values = values.clone();
    let mut subject = Subject::default();

    let subscription = observable::of(subject.clone())
      .merge_all(1)
      .subscribe(move |v| values.push(v));
    subscription.unsubscribe();

    subject.next(1);

    assert_eq!(&c_values, &[]);
  }

  #[test]
  fn it_shall_concat_all() {
    let local = Rc::new(RefCell::new(LocalPool::new()));
    let scheduler = local.borrow().spawner();
    let ticks = Rc::new(RefCell::new(Vec::<usize>::new()));

    interval(Duration::from_millis(100), scheduler.clone())
      .take(2)
      .map(move |_| {
        interval(Duration::from_millis(30), scheduler.clone()).take(5)
      })
      .concat_all()
      .subscribe({
        let ticks = Rc::clone(&ticks);
        move |v| (*ticks.borrow_mut()).push(v)
      });
    local.borrow_mut().run();
    assert_eq!(*ticks.borrow(), vec![0, 1, 2, 3, 4, 0, 1, 2, 3, 4]);
  }

  #[test]
  fn it_shall_merge_all() {
    let local = Rc::new(RefCell::new(LocalPool::new()));
    let scheduler = local.borrow().spawner();
    let ticks = Rc::new(RefCell::new(Vec::<usize>::new()));

    interval(Duration::from_millis(100), scheduler.clone())
      .take(2)
      .map(move |_| {
        interval(Duration::from_millis(30), scheduler.clone()).take(5)
      })
      .merge_all(usize::MAX)
      .subscribe({
        let ticks = Rc::clone(&ticks);
        move |v| (*ticks.borrow_mut()).push(v)
      });
    local.borrow_mut().run();
    assert_eq!(*ticks.borrow(), vec![0, 1, 2, 3, 0, 4, 1, 2, 3, 4]);
  }
}
