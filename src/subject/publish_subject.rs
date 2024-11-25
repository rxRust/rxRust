use crate::prelude::*;
use crate::rc::{AssociatedRefPtr, RcDerefMut};

#[derive(Clone)]
pub struct PublishSubject<Item, Subject: AssociatedRefPtr> {
  pub(crate) subject: Subject,
  pub(crate) value: Subject::Rc<Item>,
}

impl<Item, Subject: Default + AssociatedRefPtr> PublishSubject<Item, Subject> {
  pub fn new(value: Item) -> Self {
    Self {
      subject: <_>::default(),
      value: value.into(),
    }
  }
}

impl<Item, Err, Subject: AssociatedRefPtr> Observer<Item, Err>
  for PublishSubject<Item, Subject>
where
  Subject: Observer<Item, Err>,
  Item: Clone,
{
  #[inline]
  fn next(&mut self, value: Item) {
    *self.value.rc_deref_mut() = value.clone();
    Observer::next(&mut self.subject, value);
  }

  #[inline]
  fn error(self, err: Err) {
    self.subject.error(err)
  }

  #[inline]
  fn complete(self) {
    self.subject.complete()
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.subject.is_finished()
  }
}

impl<Item, Subject: AssociatedRefPtr> Subscription
  for PublishSubject<Item, Subject>
where
  Subject: Subscription,
{
  #[inline]
  fn unsubscribe(self) {
    self.subject.unsubscribe();
  }

  #[inline]
  fn is_closed(&self) -> bool {
    self.subject.is_closed()
  }
}

impl<Item, Subject: AssociatedRefPtr> SubjectSize
  for PublishSubject<Item, Subject>
where
  Subject: SubjectSize,
{
  #[inline]
  fn is_empty(&self) -> bool {
    self.subject.is_empty()
  }

  #[inline]
  fn len(&self) -> usize {
    self.subject.len()
  }
}

impl<Item, Err, O, Subject: AssociatedRefPtr> Observable<Item, Err, O>
  for PublishSubject<Item, Subject>
where
  Subject: Observable<Item, Err, O>,
  O: Observer<Item, Err>,
  Item: Clone,
{
  type Unsub = Subject::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self.subject.actual_subscribe(observer)
  }
}

impl<Item, Err, Subject: AssociatedRefPtr> ObservableExt<Item, Err>
  for PublishSubject<Item, Subject>
where
  Subject: ObservableExt<Item, Err>,
{
}

impl<Item, Err, Subject: AssociatedRefPtr> Publish<Item, Err>
  for PublishSubject<Item, Subject>
where
  Subject: Observer<Item, Err>,
  Item: Clone,
{
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn base_data_flow() {
    let mut i = 0;

    {
      let broadcast = PublishSubject::<_, Subject<_, _>>::new(42);
      broadcast.clone().subscribe(|v| i = v * 2);
    }

    assert_eq!(i, 0);

    {
      let mut broadcast = PublishSubject::<_, Subject<_, _>>::new(42);
      broadcast.clone().subscribe(|v| i = v * 2);
      broadcast.next(1);
    }

    assert_eq!(i, 2);
  }

  #[test]
  #[should_panic]
  fn error() {
    let broadcast = PublishSubject::<_, Subject<_, _>>::new(42);
    broadcast
      .clone()
      .on_error(|err| panic!("{}", err))
      .subscribe(|_| {});

    broadcast.error(&"should panic!");
  }

  #[test]
  fn unsubscribe() {
    let mut i = 0;

    {
      let subject = PublishSubject::<_, Subject<_, _>>::new(42);
      subject.clone().subscribe(|v| i = v).unsubscribe();
    }

    assert_eq!(i, 0);

    {
      let mut subject = PublishSubject::<_, Subject<_, _>>::new(42);
      subject.clone().subscribe(|v| i = v).unsubscribe();
      subject.next(100);
    }

    assert_eq!(i, 0);
  }

  #[test]
  fn subject_subscribe_subject() {
    let mut local = PublishSubject::<_, Subject<_, _>>::new(42);
    let local2 = PublishSubject::<_, Subject<_, _>>::new(42);
    local.clone().actual_subscribe(local2);
    local.next(1);
    local.error(2);
  }
}
