use std::marker::PhantomData;
use std::ptr::{self, NonNull};
use std::mem;
use std::cmp::Ordering;
use std::fmt;

mod node;
use node::{Node, NodeTrait};

mod cursor;
pub use cursor::Cursor;

#[repr(transparent)]
#[derive(Default)]
pub struct IterList<T: ?Sized> {
	cursor: Cursor<T>,
	_boo:   PhantomData<T>,
}

impl<T: ?Sized> std::ops::Deref for IterList<T> {
	type Target = Cursor<T>;
	fn deref(&self) -> &Self::Target 
		{ &self.cursor }
}

impl<T: ?Sized> std::ops::DerefMut for IterList<T> {
	fn deref_mut(&mut self) -> &mut Self::Target 
		{ &mut self.cursor }
}

impl<T: ?Sized> IterList<T> {
	pub fn new() -> Self {
		Self {
			cursor: Cursor::default(),
			_boo:   PhantomData,
		}
	}

	pub fn from_cursor(cursor: Cursor<T>) -> Self {
		Self { cursor, _boo: PhantomData, }
	}
}

impl<T: Sized> IterList<T> {
	pub unsafe fn new_zeroed(count: usize) -> Self {
		(0..count).fold(Self::new(), |mut list, _| {
			list.insert_next(std::mem::MaybeUninit::zeroed().assume_init()); list
		})
	}

	/// Insert an element after the cursor, retaining current position. `O(1)`.  
	/// If the list is empty it will be inserted at index 0.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::new();
	/// list.insert_next(1);
	/// list.insert_next(2);
	/// list.insert_next(3);
	/// 
	/// assert_eq!(list.current(), Some(&1));
	/// assert_eq!(&format!("{:?}", list), "[1, 3, 2]");
	/// ```
	pub fn insert_next(&mut self, elem: T) {
		let mut new = Node::new_nonnull(elem);

		match self.current {
			None => self.current = Some(new),
			Some(ref mut c) => {
				unsafe {
					if let Some(mut next) = c.as_mut().next {
						next.as_mut().prev = Some(new);
						new.as_mut().next  = Some(next);
					}

					c.as_mut().next   = Some(new);
					new.as_mut().prev = Some(*c);
				}
			},
		}

		self.len += 1;
	}

	/// Insert an element before the cursor, retaining current position. `O(1)`.  
	/// If the list is empty it will be inserted at index 0.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::new();
	/// list.insert_prev(1);
	/// list.insert_prev(2);
	/// list.insert_prev(3);
	///
	/// assert_eq!(list.current(), Some(&1));
	/// assert_eq!(&format!("{:?}", list), "[2, 3, 1]");
	/// ```
	pub fn insert_prev(&mut self, elem: T) {
		let mut new = Node::new_nonnull(elem);

		match self.current {
			None => self.current = Some(new),
			Some(ref mut c) => {
				unsafe {
					if let Some(mut prev) = c.as_mut().prev {
						prev.as_mut().next = Some(new);
						new.as_mut().prev  = Some(prev);
					}

					c.as_mut().prev   = Some(new);
					new.as_mut().next = Some(*c);
				}

				self.index += 1;
			},
		}

		self.len += 1;
	}

	/// Push an element after the cursor, moving the cursor to it. `O(1)`.  
	/// If the list is empty it will be inserted at index 0.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::new();
	/// list.push_next(1);
	/// list.push_next(2);
	/// list.push_next(3);
	///
	/// assert_eq!(format!("{:?}", list), "[1, 2, 3]");
	///
	/// assert_eq!(list.current(), Some(&3));
	/// assert_eq!(&format!("{:?}", list), "[1, 2, 3]");
	/// ```
	#[inline]
	pub fn push_next(&mut self, elem: T) {
		self.current.map_or_else(
			| | self.insert_next(elem),
			|_| { self.insert_next(elem);
				let _ = self.advance(); })
	}

	/// Push an element before the cursor, moving the cursor to it. `O(1)`.  
	/// If the list is empty it will be inserted at index 0.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::new();
	/// list.push_prev(1);
	/// list.push_prev(2);
	/// list.push_prev(3);
	///
	/// assert_eq!(list.current(), Some(&3));
	/// assert_eq!(&format!("{:?}", list), "[3, 2, 1]");
	/// ```
	#[inline]
	pub fn push_prev(&mut self, elem: T) {
		self.current.map_or_else(
			| | self.insert_prev(elem),
			|_| { self.insert_prev(elem);
				let _ = self.retreat(); })
	}
}


