use crate::{
  observable::{Observable, ObservableExt},
  observer::Observer,
};

#[derive(Clone)]
pub struct CollectOp<S, C> {
  source: S,
  collection: C,
}

impl<S, C> CollectOp<S, C> {
  pub fn new(source: S, collection: C) -> Self {
    CollectOp { source, collection }
  }
}

impl<Err, O, S, C> Observable<C, Err, O> for CollectOp<S, C>
where
  C: IntoIterator + Extend<C::Item>,
  O: Observer<C, Err>,
  S: Observable<C::Item, Err, CollectObserver<O, C>>,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    let collection = self.collection;
    self
      .source
      .actual_subscribe(CollectObserver { observer, collection })
  }
}

impl<Err, S, C> ObservableExt<C, Err> for CollectOp<S, C>
where
  C: IntoIterator + Extend<C::Item>,
  S: ObservableExt<C::Item, Err>,
{
}

#[derive(Clone)]
pub struct CollectObserver<O, C> {
  observer: O,
  collection: C,
}

impl<O, C, Item, Err> Observer<Item, Err> for CollectObserver<O, C>
where
  C: Extend<Item>,
  O: Observer<C, Err>,
{
  fn next(&mut self, value: Item) {
    self.collection.extend(Some(value));
  }

  fn error(self, err: Err) {
    self.observer.error(err);
  }

  fn complete(mut self) {
    let collection = self.collection;
    self.observer.next(collection);
    self.observer.complete();
  }

  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    prelude::*,
    rc::{MutRc, RcDeref, RcDerefMut},
  };

  #[test]
  fn collect_test() {
    let mut data = vec![];

    crate::observable::from_iter([1, 2, 3])
      .collect::<Vec<_>>()
      .subscribe(|values| {
        data = values;
      });

    assert_eq!(data, vec![1, 2, 3]);
  }

  #[test]
  fn collect_into_test() {
    let mut data = vec![];
    let base = vec![1, 2, 3];

    crate::observable::from_iter([4, 5, 6])
      .collect_into::<Vec<_>>(base)
      .subscribe(|values| {
        data = values;
      });

    assert_eq!(data, vec![1, 2, 3, 4, 5, 6]);
  }

  #[test]
  fn collect_with_err_test() {
    let mut subject = Subject::<i32, String>::default();
    let values = MutRc::own(vec![]);
    let err = MutRc::own(String::new());

    {
      let values = values.clone();
      let err = err.clone();

      subject
        .clone()
        .collect::<Vec<_>>()
        .on_error(move |e| *err.rc_deref_mut() = e)
        .subscribe(move |x| {
          values.rc_deref_mut().push(x);
        });
    }

    subject.next(2);
    subject.next(3);
    subject.next(4);
    subject.clone().error(String::from("something went wrong"));

    assert_eq!(err.rc_deref().clone(), "something went wrong");
    assert!(values.rc_deref().is_empty());
  }

  #[test]
  fn collect_empty_test() {
    let mut data = vec![];

    crate::observable::from_iter(Vec::<i32>::new())
      .collect::<Vec<_>>()
      .subscribe(|values| {
        data = values;
      });

    assert!(data.is_empty());
  }

  #[tokio::test]
  async fn collect_to_future_test() {
    let value = observable::from_iter([1, 2, 3])
      .collect::<Vec<_>>()
      .to_future()
      .await
      .unwrap()
      .unwrap();

    assert_eq!(value, vec![1, 2, 3]);
  }
}
