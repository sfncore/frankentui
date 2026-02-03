#![forbid(unsafe_code)]

//! Reactive data bindings for FrankenTUI.
//!
//! This module provides change-tracking primitives for reactive UI updates:
//!
//! - [`Observable`]: A shared, version-tracked value wrapper with change
//!   notification via subscriber callbacks.
//! - [`Subscription`]: RAII guard that automatically unsubscribes on drop.
//!
//! # Architecture
//!
//! `Observable<T>` uses `Rc<RefCell<..>>` for single-threaded shared ownership.
//! Subscribers are stored as `Weak` function pointers and cleaned up lazily
//! during notification.
//!
//! # Invariants
//!
//! 1. Version increments exactly once per mutation that changes the value.
//! 2. Subscribers are notified in registration order.
//! 3. Setting a value equal to the current value is a no-op (no version bump,
//!    no notifications).
//! 4. Dropping a [`Subscription`] removes the callback before the next
//!    notification cycle.

pub mod observable;

pub use observable::{Observable, Subscription};
