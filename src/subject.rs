use crate::{
  prelude::*,
  rc::{MutArc, MutRc, RcDeref, RcDerefMut},
  subscriber::Subscriber,
};

pub mod behavior_subject;
pub use behavior_subject::*;
use smallvec::SmallVec;

pub trait SubjectSize {
  fn is_empty(&self) -> bool;
  fn len(&self) -> usize;
}

type PublisherVec<'a, Item, Err> =
  MutRc<Option<SmallVec<[Box<dyn Publisher<Item, Err> + 'a>; 1]>>>;
pub struct Subject<'a, Item, Err> {
  observers: PublisherVec<'a, Item, Err>,
  chamber: PublisherVec<'a, Item, Err>,
}

type PublisherVecThreads<Item, Err> =
  MutArc<Option<SmallVec<[Box<dyn Publisher<Item, Err> + Send>; 1]>>>;
pub struct SubjectThreads<Item, Err> {
  observers: PublisherVecThreads<Item, Err>,
  chamber: PublisherVecThreads<Item, Err>,
}

macro_rules! impl_subject_trivial {
  ($ty: ty, $rc:ident $(,$lf:lifetime)?) => {
    impl<$($lf,)? Item, Err> Subscription for $ty {
      fn unsubscribe(self) {
        self.observers.rc_deref_mut().take();
        self.chamber.rc_deref_mut().take();
      }

      fn is_closed(&self) -> bool {
        self.observers.rc_deref().is_none()
      }
    }

    impl<$($lf,)? Item, Err>  SubjectSize for $ty {
      fn is_empty(&self) -> bool{
        self
        .observers
        .rc_deref().as_ref().map_or(true, |observers| {
          observers.is_empty()
            && self.chamber.rc_deref().as_ref().unwrap().is_empty()
        })
      }

      fn len(&self) -> usize {
        self
          .observers
          .rc_deref().as_ref().map_or(0, |observers| {
            observers.len() + self.chamber.rc_deref().as_ref().unwrap().len()
          })
      }
    }

    impl<$($lf,)? Item, Err>  Clone for $ty {
      #[inline]
      fn clone(&self) -> Self {
        Self {
          observers: self.observers.clone(),
          chamber: self.chamber.clone()
        }
      }
    }

    impl<$($lf,)? Item, Err> ObservableExt<Item, Err> for $ty {}

    impl<$($lf,)? Item, Err> Default for $ty {
      fn default() -> Self {
        Self {
          observers: $rc::own(Some(<_>::default())) ,
          chamber: $rc::own(Some(<_>::default()))
        }
      }
    }

    impl<$($lf,)? Item, Err> $ty {
      /// Retains only the subscriber that not finished.
      pub fn retain(&mut self) {
        if let Some(observers) = self.observers.rc_deref_mut().as_mut(){
          observers.retain(|p| !p.p_is_closed() );
        }
      }
      fn load(&mut self) {
        if let Some(observers) = self.observers.rc_deref_mut().as_mut() {
          observers.append(self.chamber.rc_deref_mut().as_mut().unwrap());
        }
      }
    }
  }
}
impl_subject_trivial!(Subject<'a, Item,Err>, MutRc, 'a);
impl_subject_trivial!(SubjectThreads< Item, Err>, MutArc);

macro_rules! impl_observer_methods {
  () => {
    #[inline]
    fn next(&mut self, value: Item) {
      self.load();
      if let Some(observers) = self.observers.rc_deref_mut().as_mut() {
        observers.iter_mut().for_each(|p| {
          p.p_next(value.clone());
        });
      }
    }

    #[inline]
    fn error(mut self, err: Err) {
      self.load();
      if let Some(observers) = self.observers.rc_deref_mut().take() {
        observers
          .into_iter()
          .filter(|o| !o.p_is_closed())
          .for_each(|o| o.p_error(err.clone()));
      }
    }

    #[inline]
    fn complete(mut self) {
      self.load();
      if let Some(observers) = self.observers.rc_deref_mut().take() {
        observers
          .into_iter()
          .filter(|o| !o.p_is_closed())
          .for_each(|subscriber| subscriber.p_complete());
      }
    }

    #[inline]
    fn is_finished(&self) -> bool {
      self.observers.rc_deref().is_none()
    }
  };
}

impl<'a, Item: Clone, Err: Clone> Observer<Item, Err>
  for Subject<'a, Item, Err>
{
  impl_observer_methods!();
}

impl<Item: Clone, Err: Clone> Observer<Item, Err>
  for SubjectThreads<Item, Err>
{
  impl_observer_methods!();
}

// impl<$($lf,)? $item, $err> ContextFrom for $ty {
//   type Ctx = $ctx;
// }

macro_rules! impl_observable_for_subject {
  ($subscriber:ident) => {
    type Unsub = $subscriber<O>;

    fn actual_subscribe(self, observer: O) -> Self::Unsub {
      if let Some(chamber) = self.chamber.rc_deref_mut().as_mut() {
        let subscriber = $subscriber::new(Some(observer));
        chamber.push(Box::new(subscriber.clone()));
        subscriber
      } else {
        $subscriber::new(None)
      }
    }
  };
}

impl<'a, Item, Err, O> Observable<Item, Err, O> for Subject<'a, Item, Err>
where
  O: Observer<Item, Err> + 'a,
{
  impl_observable_for_subject!(Subscriber);
}

impl<Item, Err, O> Observable<Item, Err, O> for SubjectThreads<Item, Err>
where
  O: Observer<Item, Err> + Send + 'static,
{
  impl_observable_for_subject!(SubscriberThreads);
}

// todo: support pass reference.
#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn smoke() {
    let mut value = 0;
    {
      let mut subject = Subject::default();
      subject.clone().subscribe(|v| {
        value = v;
      });
      subject.next(2);
      assert_eq!(subject.len(), 1);
    }
    assert_eq!(value, 2);
  }

  #[test]
  fn base_data_flow() {
    let mut i = 0;
    {
      let mut broadcast = Subject::default();
      broadcast.clone().subscribe(|v| i = v * 2);
      broadcast.next(1);
    }
    assert_eq!(i, 2);
  }

  #[test]
  #[should_panic]
  fn error() {
    let mut broadcast = Subject::default();
    broadcast
      .clone()
      .on_error(|err| panic!("{}", err))
      .subscribe(|_| {});

    broadcast.next(1);
    broadcast.error(&"should panic!");
  }

  #[test]
  fn unsubscribe() {
    let mut i = 0;

    {
      let mut subject = Subject::default();
      subject.clone().subscribe(|v| i = v).unsubscribe();
      subject.next(100);
    }

    assert_eq!(i, 0);
  }

  #[test]
  fn subject_subscribe_subject() {
    let mut local = Subject::default();
    let local2 = Subject::default();
    local.clone().actual_subscribe(local2);
    local.next(1);
    local.error(2);
  }
}
