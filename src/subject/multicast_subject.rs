//! Subscriber-count access for any subject usable in multicasting.

use super::{
  async_subject::{AsyncState, AsyncSubject},
  behavior_subject::BehaviorSubject,
  replay_subject::{ReplayBuffer, ReplaySubject},
  subject_core::Subject,
  subscribers::Subscribers,
};
use crate::context::RcDeref;

/// A subject that can back `ConnectableObservable` and `RefCount`.
///
/// `RefCount` connects the source when the first subscriber arrives and
/// disconnects when the last one leaves, so it needs to see the count. A
/// subject that records its terminal event reports `is_terminated` so that
/// late subscribers are served from the record instead of reconnecting the
/// source.
pub trait MulticastSubject {
  /// Number of live subscribers.
  fn subscriber_count(&self) -> usize;

  /// Whether there are no live subscribers.
  fn is_empty(&self) -> bool { self.subscriber_count() == 0 }

  /// Whether the subject has delivered a terminal event and will replay it
  /// to late subscribers instead of accepting a new source connection.
  fn is_terminated(&self) -> bool { false }
}

impl<P, O> MulticastSubject for Subject<P>
where
  P: RcDeref<Target = Subscribers<O>>,
{
  fn subscriber_count(&self) -> usize { Subject::subscriber_count(self) }
}

impl<ValuePtr, P> MulticastSubject for BehaviorSubject<ValuePtr, P>
where
  Subject<P>: MulticastSubject,
{
  fn subscriber_count(&self) -> usize { self.subject.subscriber_count() }
}

impl<P, B, Item, Err> MulticastSubject for ReplaySubject<P, B>
where
  Subject<P>: MulticastSubject,
  B: RcDeref<Target = ReplayBuffer<Item, Err>>,
{
  fn subscriber_count(&self) -> usize { self.subject.subscriber_count() }

  fn is_terminated(&self) -> bool { ReplaySubject::is_terminated(self) }
}

impl<P, V, Item, Err> MulticastSubject for AsyncSubject<P, V>
where
  Subject<P>: MulticastSubject,
  V: RcDeref<Target = AsyncState<Item, Err>>,
{
  fn subscriber_count(&self) -> usize { self.subject.subscriber_count() }

  fn is_terminated(&self) -> bool { AsyncSubject::is_terminated(self) }
}
