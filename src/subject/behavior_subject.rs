use crate::prelude::*;

#[derive(Clone)]
pub struct BehaviorSubject<Item, Subject> {
  pub(crate) subject: Subject,
  pub(crate) value: Item,
}

impl<Item, Subject: Default> BehaviorSubject<Item, Subject> {
  pub fn new(value: Item) -> Self {
    Self { subject: <_>::default(), value }
  }
}

impl<Item, Err, Subject> Observer<Item, Err> for BehaviorSubject<Item, Subject>
where
  Subject: Observer<Item, Err>,
  Item: Clone,
{
  #[inline]
  fn next(&mut self, value: Item) {
    self.value = value;
    Observer::next(&mut self.subject, self.value.clone());
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

impl<Item, Subject> Subscription for BehaviorSubject<Item, Subject>
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

impl<Item, Subject> SubjectSize for BehaviorSubject<Item, Subject>
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

impl<Item, Err, O, Subject> Observable<Item, Err, O>
  for BehaviorSubject<Item, Subject>
where
  Subject: Observable<Item, Err, O>,
  O: Observer<Item, Err>,
  Item: Clone,
{
  type Unsub = Subject::Unsub;

  fn actual_subscribe(self, mut observer: O) -> Self::Unsub {
    observer.next(self.value.clone());
    self.subject.actual_subscribe(observer)
  }
}

impl<Item, Err, Subject> ObservableExt<Item, Err>
  for BehaviorSubject<Item, Subject>
where
  Subject: ObservableExt<Item, Err>,
{
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn base_data_flow() {
    let mut i = 0;

    {
      let broadcast = BehaviorSubject::<_, Subject<_, _>>::new(42);
      broadcast.clone().subscribe(|v| i = v * 2);
    }

    assert_eq!(i, 84);

    {
      let mut broadcast = BehaviorSubject::<_, Subject<_, _>>::new(42);
      broadcast.clone().subscribe(|v| i = v * 2);
      broadcast.next(1);
    }

    assert_eq!(i, 2);
  }

  #[test]
  #[should_panic]
  fn error() {
    let broadcast = BehaviorSubject::<_, Subject<_, _>>::new(42);
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
      let subject = BehaviorSubject::<_, Subject<_, _>>::new(42);
      subject.clone().subscribe(|v| i = v).unsubscribe();
    }

    assert_eq!(i, 42);

    {
      let mut subject = BehaviorSubject::<_, Subject<_, _>>::new(42);
      subject.clone().subscribe(|v| i = v).unsubscribe();
      subject.next(100);
    }

    assert_eq!(i, 42);
  }

  #[test]
  fn subject_subscribe_subject() {
    let mut local = BehaviorSubject::<_, Subject<_, _>>::new(42);
    let local2 = BehaviorSubject::<_, Subject<_, _>>::new(42);
    local.clone().actual_subscribe(local2);
    local.next(1);
    local.error(2);
  }
}
