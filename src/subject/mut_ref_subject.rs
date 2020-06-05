use crate::prelude::*;
use observer::{complete_proxy_impl, error_proxy_impl, next_proxy_impl};

pub struct MutRefValue<T>(pub *mut T);

impl<T> Clone for MutRefValue<T> {
  #[inline]
  fn clone(&self) -> Self { MutRefValue(self.0) }
}

impl<T> Copy for MutRefValue<T> {}

#[derive(Clone)]
pub struct MutRefSubject<'a, Item, Err, MI, ME> {
  subject: LocalSubject<'a, Item, Err>,
  map_item: MI,
  map_err: ME,
}

subscription_proxy_impl!(
  MutRefSubject<'a, Item, Err, MI, ME>, {subject}, <'a, Item, Err, MI, ME>);

#[inline]
fn value_as_mut_ref<'a, T>(v: MutRefValue<T>) -> &'a mut T {
  unsafe { &mut *v.0 }
}
#[inline]
fn none_map<T>(v: T) -> T { v }

pub type MutRefItemSubject<'a, Item, Err> = MutRefSubject<
  'a,
  MutRefValue<Item>,
  Err,
  fn(MutRefValue<Item>) -> &'a mut Item,
  fn(Err) -> Err,
>;
impl<'a, Item, Err> LocalSubject<'a, MutRefValue<Item>, Err> {
  /// # Safety
  /// You should know MutRefSubject will be erased the life time of your mut
  /// ref value.
  pub unsafe fn mut_ref_item(self) -> MutRefItemSubject<'a, Item, Err> {
    MutRefSubject {
      subject: self,
      map_item: value_as_mut_ref,
      map_err: none_map,
    }
  }
}

pub type MutRefErrSubject<'a, Item, Err> = MutRefSubject<
  'a,
  Item,
  MutRefValue<Err>,
  fn(Item) -> Item,
  fn(MutRefValue<Err>) -> &'a mut Err,
>;
impl<'a, Item, Err> LocalSubject<'a, Item, MutRefValue<Err>> {
  /// # Safety
  /// You should know MutRefSubject will be erased the life time of your mut ref
  /// error.
  pub unsafe fn mut_ref_err(self) -> MutRefErrSubject<'a, Item, Err> {
    MutRefSubject {
      subject: self,
      map_item: none_map,
      map_err: value_as_mut_ref,
    }
  }
}

pub type MutRefAllSubject<'a, Item, Err> = MutRefSubject<
  'a,
  MutRefValue<Item>,
  MutRefValue<Err>,
  fn(MutRefValue<Err>) -> &'a mut Err,
  fn(MutRefValue<Item>) -> &'a mut Item,
>;
impl<'a, Item, Err> LocalSubject<'a, MutRefValue<Item>, MutRefValue<Err>> {
  /// # Safety
  /// You should know MutRefSubject will be erased the life time of your mut ref
  /// value and error.
  pub unsafe fn mut_ref_all(self) -> MutRefAllSubject<'a, Item, Err> {
    MutRefSubject {
      subject: self,
      map_item: value_as_mut_ref,
      map_err: value_as_mut_ref,
    }
  }
}

impl<'a, Item, Err: Clone> Observer<&mut Item, Err>
  for MutRefSubject<
    'a,
    MutRefValue<Item>,
    Err,
    fn(MutRefValue<Item>) -> &'a mut Item,
    fn(Err) -> Err,
  >
{
  #[inline]
  fn next(&mut self, value: &mut Item) { self.subject.next(MutRefValue(value)) }
  error_proxy_impl!(Err, subject);
  complete_proxy_impl!(subject);
}

impl<'a, Item: Clone, Err> Observer<Item, &mut Err>
  for MutRefSubject<
    'a,
    Item,
    MutRefValue<Err>,
    fn(Item) -> Item,
    fn(MutRefValue<Err>) -> &'a mut Err,
  >
{
  next_proxy_impl!(Item, subject);
  #[inline]
  fn error(&mut self, err: &mut Err) { self.subject.error(MutRefValue(err)) }
  complete_proxy_impl!(subject);
}

impl<'a, Item, Err> Observer<&mut Item, &mut Err>
  for MutRefSubject<
    'a,
    MutRefValue<Item>,
    MutRefValue<Err>,
    fn(MutRefValue<Err>) -> &'a mut Err,
    fn(MutRefValue<Item>) -> &'a mut Item,
  >
{
  #[inline]
  fn next(&mut self, value: &mut Item) { self.subject.next(MutRefValue(value)) }
  #[inline]
  fn error(&mut self, err: &mut Err) { self.subject.error(MutRefValue(err)) }
  complete_proxy_impl!(subject);
}

struct MutRefObserver<O, MI, ME> {
  observer: O,
  map_item: MI,
  map_err: ME,
}

impl<'a, MI, ME, Item, Err, MapItem, MapErr> Observable
  for MutRefSubject<'a, Item, Err, MI, ME>
where
  MI: Fn(Item) -> MapItem + 'a,
  ME: Fn(Err) -> MapErr + 'a,
{
  type Item = MapItem;
  type Err = MapErr;
}

impl<'a, MI, ME, Item, Err, MapItem, MapErr> LocalObservable<'a>
  for MutRefSubject<'a, Item, Err, MI, ME>
where
  MI: Fn(Item) -> MapItem + 'a,
  ME: Fn(Err) -> MapErr + 'a,
{
  type Unsub = LocalSubscription;
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'a>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> LocalSubscription {
    self.subject.actual_subscribe(Subscriber {
      observer: MutRefObserver {
        observer: subscriber.observer,
        map_item: self.map_item,
        map_err: self.map_err,
      },
      subscription: subscriber.subscription,
    })
  }
}

impl<O, MI, ME, Item, Err, MapItem, MapErr> Observer<Item, Err>
  for MutRefObserver<O, MI, ME>
where
  MI: Fn(Item) -> MapItem,
  ME: Fn(Err) -> MapErr,
  O: Observer<MapItem, MapErr>,
{
  fn next(&mut self, v: Item) { self.observer.next((self.map_item)(v)) }
  fn error(&mut self, err: Err) { self.observer.error((self.map_err)(err)) }
  complete_proxy_impl!(observer);
}

#[test]
fn mut_ref_item() {
  let mut test_code = 0;
  {
    let mut subject = unsafe { Subject::new().mut_ref_item() };
    subject.clone().subscribe(|v: &mut i32| {
      *v = 100;
    });
    subject.next(&mut test_code);
  }
  assert_eq!(test_code, 100);
}

#[test]
fn mut_ref_err() {
  let mut test_code = 0;
  {
    let mut subject = unsafe { Subject::new().mut_ref_err() };
    subject.clone().subscribe_err(
      |_: i32| {},
      |v: &mut i32| {
        *v = 100;
      },
    );
    subject.error(&mut test_code);
  }
  assert_eq!(test_code, 100);
}
