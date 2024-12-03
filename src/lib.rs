//! A doubly linked list with a cursor based api.  
//! *it's also an iterator!*  
//! 
//! `O(1)` pretty much everything (at the cursor).  
//! 
//! ## Example
//! 
//! ```rust
//! use iterlist::IterList;
//! 
//! let mut list = IterList::new();
//! 
//! list.push_prev(-1);
//! list.push_next(1);
//! list.push_next(2);
//! list.push_next(3);
//! 
//! assert_eq!(format!("{:?}", list), "[-1, 1, 2, 3]");
//! 
//! list.move_to(2);
//! assert_eq!(list.current(), Some(&2));
//! 
//! list.move_by(-2);
//! assert_eq!(list.index(), 0);
//! 
//! let mut cursor = list.as_cursor();
//! assert_eq!(cursor.next(), Some(&-1));
//! assert_eq!(cursor.next(), Some(&1));
//! 
//! assert_eq!(list.get(1), Some(&1));
//! 
//! list.move_by(2);
//! let (elem, _) = list.consume_forward().unwrap();
//! assert_eq!(elem, 2);
//! 
//! assert_eq!(format!("{:?}", list), "[-1, 1, 3]");
//! 
//! let num = list.fold(0, |acc, elem| acc + elem);
//! 
//! assert_eq!(num, 3);
//! ```

// #![feature(min_specialization)]

#![allow(forbidden_lint_groups)]
#![forbid(clippy::all)]
#![allow(clippy::option_map_unit_fn, clippy::wrong_self_convention, clippy::uninit_assumed_init)]

#[cfg(feature = "atomic")]
pub mod atomic;

#[cfg(not(feature = "nightly"))]
mod list;
#[cfg(not(feature = "nightly"))]
pub use list::{IterList, Cursor};

#[cfg(feature = "nightly")]
mod nightly;
#[cfg(feature = "nightly")]
pub use nightly::{IterList, Cursor};

