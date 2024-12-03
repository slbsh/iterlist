use std::ptr::NonNull;
use std::cmp::Ordering;
use std::mem;

use super::node::Node;

pub struct Cursor<T: ?Sized> {
	current: Option<NonNull<Node<T>>>,
	index:   usize,
	len:     usize,
}

impl<T: ?Sized> Default for Cursor<T> {
	fn default() -> Self {
		Self {
			current: None,
			index:   0,
			len:     0,
		}
	}
}

impl<T: Sized> Cursor<T> {
	fn dangling() -> Self {
		Self {
			current: Some(NonNull::dangling()),
			.. Self::default()
		}
	}
}

impl<T: ?Sized> Cursor<T> {
	/// Move the cursor to the front of the list. `O(n)`.  
	/// Returns the number of elements traversed.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::new();
	/// list.push_next(1);
	/// list.push_next(2);
	/// list.push_next(3);
	///
	/// assert_eq!(list.current(), Some(&3));
	///
	/// let offset = list.move_to_front();
	/// assert_eq!(offset, 2);
	/// assert_eq!(list.current(), Some(&1));
	/// ```
	pub fn goto_start(&mut self) -> usize {
		self.current.map_or(0, |ref mut c| {
			self.index = 0;

			for i in 0_usize.. {
				match unsafe { c.as_ref().prev } {
					Some(prev) => *c = prev,
					None => return i,
				}
			}

			unsafe { std::hint::unreachable_unchecked() }
		})
	}

	/// Move the cursor to the back of the list. `O(n)`.  
	/// Returns the number of elements traversed.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	///
	/// let offset = list.move_to_back();
	/// assert_eq!(offset, 2);
	/// assert_eq!(list.current(), Some(&3));
	/// ```
	pub fn goto_end(&mut self) -> usize {
		self.current.map_or(0, |ref mut c| {
			self.index = self.len - 1;

			for i in 0_usize.. {
				match unsafe { c.as_ref().next } {
					Some(next) => *c = next,
					None => return i,
				}
			}

			unsafe { std::hint::unreachable_unchecked() }
		})
	}

	/// Move the cursor to the specified index. `O(n)`.
	/// If the index is out of bounds the cursor will be moved to the edge, 
	/// and `false` will be returned.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	/// list.move_to(1);
	/// assert_eq!(list.current(), Some(&2));
	/// ```
	#[inline]
	#[must_use]
	pub fn move_to(&mut self, index: usize) -> bool {
		match self.index.cmp(&index) {
			Ordering::Greater => !(0..self.index - index).any(|_| !self.retreat()),
			Ordering::Less    => !(0..index - self.index).any(|_| !self.advance()),
			Ordering::Equal   => true,
		}
	}

	/// Move the cursor one step forward. `O(1)`.  
	/// Returns `false` if the cursor could not be moved.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	/// assert_eq!(list.current(), Some(&1));
	///
	/// list.advance();
	/// assert_eq!(list.current(), Some(&2));
	/// ```
	#[inline]
	#[must_use]
	pub fn advance(&mut self) -> bool {
		self.current.map(|ref mut c| 
			unsafe { c.as_ref() }.next.map(|n| { *c = n; self.index += 1; }))
			.is_some()
	}

	// /// Move the cursor one step forward. `O(1)`.  
	// ///
	// /// # Safety
	// /// This function assumes that the cursor is not at the end of the list,
	// /// it is up to the caller to ensure this.
	// #[inline]
	// pub unsafe fn advance_unchecked(&mut self) {
	// 	self.current = self.current.as_ref().next.unwrap_unchecked();
	// 	self.index += 1;
	// }
	//
	// /// Move the cursor one step backward. `O(1)`.  
	// /// Returns `false` if the cursor could not be moved.
	// /// ```
	// /// # use iterlist::IterList;
	// /// let mut list = IterList::from(vec![1, 2, 3]);
	// ///
	// /// list.move_to_back();
	// /// assert_eq!(list.current(), Some(&3));
	// ///
	// /// list.retreat();
	// /// assert_eq!(list.current(), Some(&2));
	// /// ```
	// #[inline]
	// #[must_use]
	// pub fn retreat(&mut self) -> bool {
	// 	if self.len == 0 { return false; }
	//
	// 	unsafe { self.current.as_ref() }.prev.map(|prev| {
	// 			self.current = prev;
	// 			self.index -= 1; })
	// 		.is_some()
	// }
	//
	// /// Move the cursor one step backward. `O(1)`.  
	// ///
	// /// # Safety
	// /// This function assumes that the cursor is not at the end of the list,
	// /// it is up to the caller to ensure this.
	// #[inline]
	// pub unsafe fn retreat_unchecked(&mut self) {
	// 	self.current = self.current.as_ref().prev.unwrap_unchecked();
	// 	self.index -= 1;
	// }
	//
	// /// Move the cursor by a given offset. `O(n)`.  
	// /// If the offset is out of bounds the cursor will be moved to the edge, 
	// /// and `false` will be returned.
	// /// ```
	// /// # use iterlist::IterList;
	// /// let mut list = IterList::from(vec![1, 2, 3]);
	// ///
	// /// list.move_to_back();
	// /// assert_eq!(list.index(), 2);
	// ///
	// /// list.move_by(-2);
	// /// assert_eq!(list.index(), 0);
	// /// assert_eq!(list.current(), Some(&1));
	// ///
	// /// assert!(!list.move_by(10));
	// /// assert_eq!(list.index(), 2);
	// /// ```
	// #[inline]
	// #[must_use]
	// pub fn move_by(&mut self, offset: isize) -> bool {
	// 	match offset.cmp(&0) {
	// 		Ordering::Greater => (0..offset ).fold(true, |_, _| self.advance()),
	// 		Ordering::Less    => (0..-offset).fold(true, |_, _| self.retreat()),
	// 		Ordering::Equal   => true,
	// 	}
	// }
}

impl<T: ?Sized> Drop for Cursor<T> {
	/// Drop the list. `O(n)`.
	fn drop(&mut self) {
		let Some(mut current) = self.current else { return; };

		if self.index < self.len - self.index {
			self.goto_start();
			loop {
				let next = unsafe { current.as_ref().next };
				mem::drop(unsafe { Box::from_raw(current.as_ptr()) });
				match next {
					Some(next) => current = next,
					None       => break,
				}
			}
			return;
		}

		self.goto_end();
		loop {
			let prev = unsafe { current.as_ref().prev };
			mem::drop(unsafe { Box::from_raw(current.as_ptr()) });
			match prev {
				Some(prev) => current = prev,
				None       => break,
			}
		}
	}
}
