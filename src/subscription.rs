use crate::Observer;
/// Subscription returns from `Observable.subscribe(Subscriber)` to allow
///  unsubscribing.
pub trait Subscription {
  /// This allows deregistering an stream before it has finished receiving all
  ///  events (i.e. before onCompleted is called).
  fn unsubscribe(&mut self);
}

pub trait Publisher: Subscription + Observer {}
