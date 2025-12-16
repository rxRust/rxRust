//! Rc wrapper types for shared mutable state
//!
//! This module provides wrapper types for reference-counted shared mutable
//! state, used by the Context system for managing observer and subscription
//! sharing.
//!
//! ## Types
//!
//! | Type | Local | Shared | Use Case |
//! |------|-------|--------|----------|
//! | `MutRc<T>` / `MutArc<T>` | `Rc<RefCell<T>>` | `Arc<Mutex<T>>` | Complex mutable state |
//! | `CellRc<T>` / `CellArc<T>` | `Rc<Cell<T>>` | `Arc<AtomicCell<T>>` | Simple Copy flags/counters |

// Standard library imports
use std::{
  cell::{Cell, Ref, RefCell, RefMut},
  ops::{Deref, DerefMut},
  sync::{Mutex, MutexGuard},
};

// External crate imports
use crossbeam_utils::atomic::AtomicCell;
use rclite::{Arc, Rc};

// ==================== Traits ====================

/// Trait for dereferencing shared pointers (read-only)
pub trait RcDeref: Clone {
  type Target;
  type Ref<'a>: Deref<Target = Self::Target>
  where
    Self: 'a;

  fn rc_deref(&self) -> Self::Ref<'_>;
}

/// Trait for dereferencing shared pointers (mutable)
pub trait RcDerefMut: RcDeref {
  type MutRef<'a>: DerefMut<Target = Self::Target>
  where
    Self: 'a;

  fn rc_deref_mut(&self) -> Self::MutRef<'_>;

  /// Try to obtain a mutable guard without panicking (RefCell) or blocking
  /// (Mutex).
  ///
  /// This is primarily used to detect synchronous re-entrancy so callers can
  /// defer work via a scheduler instead of panicking or deadlocking.
  fn try_rc_deref_mut(&self) -> Option<Self::MutRef<'_>>;
}

/// Trait for efficient shared mutable access to Copy types
///
/// This trait provides a simpler interface than `RcDerefMut` for types that
/// only need get/set semantics without borrow guards. Used for flags, counters,
/// etc.
pub trait SharedCell<T: Copy + Eq>: Clone + From<T> {
  fn get(&self) -> T;
  fn set(&self, value: T);
  fn compare_exchange(&self, current: T, new: T) -> Result<T, T>;
}

// ==================== Local Types ====================

/// MutRc: Rc-based mutable sharing for Local context
///
/// Wraps `Rc<RefCell<T>>` for single-threaded shared mutable state.
pub struct MutRc<T>(Rc<RefCell<T>>);

impl<T> Clone for MutRc<T> {
  fn clone(&self) -> Self { Self(self.0.clone()) }
}

impl<T> From<T> for MutRc<T> {
  fn from(v: T) -> Self { Self(Rc::new(RefCell::new(v))) }
}

impl<T: Default> Default for MutRc<T> {
  fn default() -> Self { T::default().into() }
}

impl<T> RcDeref for MutRc<T> {
  type Target = T;
  type Ref<'a>
    = Ref<'a, T>
  where
    Self: 'a;

  fn rc_deref(&self) -> Self::Ref<'_> { self.0.borrow() }
}

impl<T> RcDerefMut for MutRc<T> {
  type MutRef<'a>
    = RefMut<'a, T>
  where
    Self: 'a;

  fn rc_deref_mut(&self) -> Self::MutRef<'_> { self.0.borrow_mut() }

  fn try_rc_deref_mut(&self) -> Option<Self::MutRef<'_>> { self.0.try_borrow_mut().ok() }
}

/// CellRc: Rc-based cell sharing for Copy types in Local context
///
/// More efficient than MutRc for simple flags/counters since it uses Cell
/// instead of RefCell, avoiding runtime borrow checking overhead.
pub struct CellRc<T>(Rc<Cell<T>>);

impl<T: Copy> Clone for CellRc<T> {
  fn clone(&self) -> Self { Self(self.0.clone()) }
}

impl<T: Copy> From<T> for CellRc<T> {
  fn from(v: T) -> Self { Self(Rc::new(Cell::new(v))) }
}

impl<T: Copy + Default> Default for CellRc<T> {
  fn default() -> Self { T::default().into() }
}

impl<T: Copy + Eq> SharedCell<T> for CellRc<T> {
  fn get(&self) -> T { self.0.get() }

  fn set(&self, value: T) { self.0.set(value) }

  fn compare_exchange(&self, current: T, new: T) -> Result<T, T> {
    let old = self.0.get();
    if old == current {
      self.0.set(new);
      Ok(old)
    } else {
      Err(old)
    }
  }
}

// ==================== Shared Types ====================

/// MutArc: Arc-based thread-safe mutable sharing for Shared context
///
/// Wraps `Arc<Mutex<T>>` for multi-threaded shared mutable state.
pub struct MutArc<T>(Arc<Mutex<T>>);

impl<T> Clone for MutArc<T> {
  fn clone(&self) -> Self { Self(self.0.clone()) }
}

impl<T> From<T> for MutArc<T> {
  fn from(v: T) -> Self { Self(Arc::new(Mutex::new(v))) }
}

impl<T: Default> Default for MutArc<T> {
  fn default() -> Self { T::default().into() }
}

impl<T> RcDeref for MutArc<T> {
  type Target = T;
  type Ref<'a>
    = MutexGuard<'a, T>
  where
    Self: 'a;

  fn rc_deref(&self) -> Self::Ref<'_> { self.0.lock().unwrap() }
}

impl<T> RcDerefMut for MutArc<T> {
  type MutRef<'a>
    = MutexGuard<'a, T>
  where
    Self: 'a;

  fn rc_deref_mut(&self) -> Self::MutRef<'_> { self.0.lock().unwrap() }

  fn try_rc_deref_mut(&self) -> Option<Self::MutRef<'_>> { self.0.try_lock().ok() }
}

/// CellArc: Arc-based atomic cell for Copy types in Shared context
///
/// Uses crossbeam's AtomicCell for lock-free atomic operations.
/// More efficient than MutArc for simple flags/counters.
pub struct CellArc<T>(Arc<AtomicCell<T>>);

impl<T: Copy> Clone for CellArc<T> {
  fn clone(&self) -> Self { Self(self.0.clone()) }
}

impl<T: Copy> From<T> for CellArc<T> {
  fn from(v: T) -> Self { Self(Arc::new(AtomicCell::new(v))) }
}

impl<T: Copy + Default> Default for CellArc<T> {
  fn default() -> Self { T::default().into() }
}

impl<T: Copy + Eq> SharedCell<T> for CellArc<T> {
  fn get(&self) -> T { self.0.load() }

  fn set(&self, value: T) { self.0.store(value) }

  fn compare_exchange(&self, current: T, new: T) -> Result<T, T> {
    self.0.compare_exchange(current, new)
  }
}
