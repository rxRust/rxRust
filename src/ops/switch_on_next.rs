use crate::observer::error_proxy_impl;

use crate::prelude::*;
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

// TODO In the final implementation, this should also be generic on error type.
// Right  now it assumes the error type to be ().
#[derive(Clone)]
pub struct SwitchOnNextOp<S, InnerItem> {
  pub(crate) source: S,
  pub(crate) p: PhantomData<InnerItem>,
}

impl<S, InnerItem> Observable for SwitchOnNextOp<S, InnerItem>
where
  S: Observable,
{
  type Item = InnerItem;
  type Err = ();
}

// TODO Also create a thread-safe version.
impl<'a, SourceObservable, InnerObservable, InnerItem> LocalObservable<'a>
  for SwitchOnNextOp<SourceObservable, InnerItem>
where
  SourceObservable: LocalObservable<'a, Item = InnerObservable, Err = ()> + 'a,
  InnerObservable: LocalObservable<'a, Item = InnerItem, Err = ()> + 'a,
{
  type Unsub = LocalSubscription;

  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'a>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub {
    let mut subscription = subscriber.subscription;
    let inner_subscription = LocalSubscription::default();
    // We need to "hand out" ownership of the observer multiple times (whenever
    // the outer observable emits a new inner observable), so we need to
    // make it shared.
    let subscriber = Subscriber {
      observer: SwitchOnNextObserver {
        observer: Rc::new(RefCell::new(subscriber.observer)),
        subscription: subscription.clone(),
        inner_subscription,
        one_is_complete: Rc::new(Cell::new(false)),
      },
      subscription: subscription.clone(),
    };
    subscription.add(self.source.actual_subscribe(subscriber));
    subscription
  }
}

#[derive(Clone)]
pub struct SwitchOnNextObserver<O> {
  observer: Rc<RefCell<O>>,
  subscription: LocalSubscription,
  inner_subscription: LocalSubscription,
  one_is_complete: Rc<Cell<bool>>,
}

#[derive(Clone)]
struct InnerObserver<O> {
  observer: Rc<RefCell<O>>,
  one_is_complete: Rc<Cell<bool>>,
}

impl<O, Item, Err> Observer<Item, Err> for InnerObserver<O>
where
  O: Observer<Item, Err>,
{
  #[inline]
  fn next(&mut self, value: Item) { self.observer.next(value); }

  error_proxy_impl!(Err, observer);

  #[inline]
  fn complete(&mut self) {
    println!("inner complete");
    if self.one_is_complete.replace(true) {
      println!("inner actual complete");
      self.observer.complete();
    }
  }
}

// TODO Is `Item` bound correct or too restrictive?
impl<'a, Item, InnerItem, O> Observer<Item, ()> for SwitchOnNextObserver<O>
where
  O: Observer<InnerItem, ()> + 'a,
  Item: LocalObservable<'a, Item = InnerItem, Err = ()>,
{
  #[inline]
  fn next(&mut self, value: Item) {
    // Unsubscribe from previous inner observable (if any)
    self.inner_subscription.unsubscribe();
    // Create a new inner subscription
    let inner_subscription = LocalSubscription::default();
    self.inner_subscription = inner_subscription.clone();
    // Make the inner subscription end when the outer one ends
    self.subscription.add(inner_subscription.clone());
    // Reset completion
    println!("reset completion");
    self.one_is_complete.set(false);
    // Subscribe
    value.actual_subscribe(Subscriber {
      observer: InnerObserver {
        observer: self.observer.clone(),
        one_is_complete: self.one_is_complete.clone(),
      },
      subscription: inner_subscription,
    });
  }

  #[inline]
  fn error(&mut self, _: ()) {
    self.inner_subscription.unsubscribe();
    self.observer.error(());
  }

  #[inline]
  fn complete(&mut self) {
    println!("outer complete");
    if self.one_is_complete.replace(true) {
      println!("outer actual complete");
      self.observer.complete();
    }
  }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use std::cell::RefCell;
  use std::rc::Rc;

  /// Reflects the events that can happen on observables.
  #[derive(Eq, PartialEq, Debug)]
  enum Event<Item, Err> {
    Next(Item),
    Error(Err),
    Complete,
  }

  /// A buffer for testing observables.
  #[derive(Default)]
  struct EventBuffer<Item, Err> {
    buffer: RefCell<Vec<Event<Item, Err>>>,
  }

  impl<Item, Err> EventBuffer<Item, Err> {
    fn next(&self, item: Item) {
      self.buffer.borrow_mut().push(Event::Next(item));
    }

    #[allow(unused)]
    fn error(&self, err: Err) {
      self.buffer.borrow_mut().push(Event::Error(err));
    }

    fn complete(&self) { self.buffer.borrow_mut().push(Event::Complete); }

    /// Empties buffer and returns current content.
    fn pop(&self) -> Vec<Event<Item, Err>> {
      self.buffer.replace(Default::default())
    }
  }

  #[test]
  fn base_function() {
    // Given
    let buffer: Rc<EventBuffer<i32, ()>> = Default::default();
    let mut s = LocalSubject::new();
    let ranges = s.clone().map(|i| observable::from_iter(i..(i + 3)));
    // When
    let _ = {
      let bc1 = buffer.clone();
      let bc2 = buffer.clone();
      ranges
        .switch_on_next()
        .subscribe_complete(move |i| bc1.next(i), move || bc2.complete())
    };
    // Then
    use Event::*;
    s.next(0);
    assert_eq!(buffer.pop(), vec![Next(0), Next(1), Next(2)]);
    s.next(10);
    assert_eq!(buffer.pop(), vec![Next(10), Next(11), Next(12)]);
    s.next(100);
    assert_eq!(buffer.pop(), vec![Next(100), Next(101), Next(102)]);
    s.complete();
    assert_eq!(buffer.pop(), vec![Complete]);
  }

  #[test]
  fn completion_details() {
    // Given
    let buffer: Rc<EventBuffer<&'static str, ()>> = Default::default();
    let mut outer = LocalSubject::new();
    let mut a = LocalSubject::new();
    let mut b = LocalSubject::new();
    let mut c = LocalSubject::new();
    let ranges = {
      let a_clone = a.clone();
      let b_clone = b.clone();
      let c_clone = c.clone();
      outer.clone().map(move |i| match i {
        "a" => a_clone.clone(),
        "b" => b_clone.clone(),
        _ => c_clone.clone(),
      })
    };
    // When
    let _ = {
      let bc1 = buffer.clone();
      let bc2 = buffer.clone();
      ranges
        .switch_on_next()
        .subscribe_complete(move |i| bc1.next(i), move || bc2.complete())
    };
    // Then
    use Event::*;
    // a
    outer.next("a");
    a.next("a1");
    a.next("a2");
    // b
    outer.next("b");
    a.next("a3");
    a.complete();
    b.next("b1");
    b.next("b2");
    b.complete();
    c.next("c1");
    // c
    outer.next("c");
    c.next("c2");
    outer.complete();
    c.next("c3");
    c.complete();
    // TODO This doesn't send a Complete. It's unclear to me if it should.
    //  RxJava says that we complete if both the outer and inner observable
    //  complete. Here the outer  observable completed *before* the inner one
    //  (c) and with this implementation it doesn't fire.  Should it?
    assert_eq!(
      buffer.pop(),
      vec![Next("a1"), Next("a2"), Next("b1"), Next("b2"), Next("c2"),]
    );
  }

  #[test]
  fn unsubscribe_details() {
    // Given
    let buffer: Rc<EventBuffer<&'static str, ()>> = Default::default();
    let mut outer = LocalSubject::new();
    let mut a = LocalSubject::new();
    let mut b = LocalSubject::new();
    let mut c = LocalSubject::new();
    let ranges = {
      let a_clone = a.clone();
      let b_clone = b.clone();
      let c_clone = c.clone();
      outer.clone().map(move |i| match i {
        "a" => a_clone.clone(),
        "b" => b_clone.clone(),
        _ => c_clone.clone(),
      })
    };
    // When
    let mut subscription = {
      let bc1 = buffer.clone();
      let bc2 = buffer.clone();
      ranges
        .switch_on_next()
        .subscribe_complete(move |i| bc1.next(i), move || bc2.complete())
    };
    // Then
    use Event::*;
    // a
    outer.next("a");
    a.next("a1");
    a.next("a2");
    // b
    outer.next("b");
    a.next("a3");
    a.complete();
    b.next("b1");
    subscription.unsubscribe();
    b.next("b2");
    b.complete();
    c.next("c1");
    // c
    outer.next("c");
    c.next("c2");
    outer.complete();
    c.next("c3");
    c.complete();
    assert_eq!(buffer.pop(), vec![Next("a1"), Next("a2"), Next("b1"),]);
  }
}
