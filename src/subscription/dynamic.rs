use smallvec::SmallVec;

/// A container for managing multiple subscriptions with ID-based tracking.
///
/// This struct provides a common abstraction for scenarios that need to:
/// - Store multiple subscriptions/items dynamically
/// - Add new items and get a unique ID
/// - Remove specific items by ID (e.g., when an inner observable completes)
/// - Unsubscribe all items at once
///
/// # Design
///
/// - **SmallVec Optimization**: Uses `SmallVec<[_; 2]>` to avoid heap
///   allocation for the common case of 0-2 items.
/// - **Pre-allocation Pattern**: Supports `reserve_id()` + `insert()` to handle
///   cyclic dependencies where the ID is needed before the item exists.
///
/// # Examples
///
/// ```rust
/// use rxrust::subscription::DynamicSubscriptions;
///
/// let mut subs: DynamicSubscriptions<()> = DynamicSubscriptions::default();
///
/// // Normal add pattern
/// let id1 = subs.add(());
/// assert_eq!(subs.len(), 1);
///
/// // Pre-allocation pattern (for cyclic dependencies)
/// let id2 = subs.reserve_id();
/// // ... create observer with id2 ...
/// // ... subscribe and get subscription ...
/// subs.insert(id2, ());
/// assert_eq!(subs.len(), 2);
///
/// // Remove by ID
/// assert!(subs.remove(id1).is_some());
/// assert_eq!(subs.len(), 1);
/// ```
pub struct DynamicSubscriptions<U> {
  next_id: usize,
  items: SmallVec<[(usize, U); 2]>,
}

impl<U> Default for DynamicSubscriptions<U> {
  fn default() -> Self { Self { next_id: 0, items: SmallVec::new() } }
}

impl<U> DynamicSubscriptions<U> {
  /// Create an empty container.
  #[inline]
  pub fn new() -> Self { Self::default() }

  /// Add an item and return its unique ID.
  #[inline]
  pub fn add(&mut self, item: U) -> usize {
    let id = self.next_id;
    self.next_id += 1;
    self.items.push((id, item));
    id
  }

  /// Reserve the next ID without adding an item.
  ///
  /// Use this with `insert()` when you need the ID before the item exists
  /// (e.g., for cyclic dependencies in observer patterns).
  #[inline]
  pub fn reserve_id(&mut self) -> usize {
    let id = self.next_id;
    self.next_id += 1;
    id
  }

  /// Insert an item with a pre-reserved ID.
  ///
  /// The ID should have been obtained from `reserve_id()`.
  #[inline]
  pub fn insert(&mut self, id: usize, item: U) { self.items.push((id, item)); }

  /// Remove an item by ID. Returns true if found and removed.
  pub fn remove(&mut self, id: usize) -> Option<U> {
    self
      .items
      .iter()
      .position(|(i, _)| *i == id)
      .map(|pos| self.items.remove(pos).1)
  }

  /// Check if an ID exists in the container.
  #[inline]
  pub fn contains(&self, id: usize) -> bool { self.items.iter().any(|(i, _)| *i == id) }

  /// Get the number of items.
  #[inline]
  pub fn len(&self) -> usize { self.items.len() }

  /// Check if empty.
  #[inline]
  pub fn is_empty(&self) -> bool { self.items.is_empty() }

  /// Drain all items.
  #[inline]
  pub fn drain(&mut self) -> impl Iterator<Item = U> + '_ {
    self.items.drain(..).map(|(_, item)| item)
  }

  /// Iterate over all items.
  #[inline]
  pub fn iter(&self) -> impl Iterator<Item = &U> { self.items.iter().map(|(_, item)| item) }

  /// Iterate over all items mutably.
  #[inline]
  pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut U> {
    self.items.iter_mut().map(|(_, item)| item)
  }
}

use super::Subscription;

impl<U: Subscription> DynamicSubscriptions<U> {
  /// Unsubscribe all items and clear the container.
  #[inline]
  pub fn unsubscribe_all(&mut self) {
    for item in self.drain() {
      item.unsubscribe();
    }
  }

  /// Check if all items are closed.
  #[inline]
  pub fn all_closed(&self) -> bool {
    self
      .items
      .iter()
      .all(|(_, item)| item.is_closed())
  }
}
