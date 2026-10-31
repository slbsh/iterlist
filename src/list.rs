use std::marker::PhantomData;
use std::ptr::NonNull;
use std::mem;
use std::cmp::Ordering;
use std::fmt;

use crate::node::{Node, NodeTrait};
use crate::cursor::Cursor;

/// A doubly linked list. Exactly the same memory layout as `Cursor`.
pub struct IterList<T: ?Sized> {
	current: Option<NonNull<Node<T>>>,
	index:   usize,
	len:     usize,
	_boo:    PhantomData<T>,
}

impl<T: ?Sized> Default for IterList<T> {
	#[inline]
	fn default() -> Self {
		Self::new()
	}
}

impl<T: Clone> IterList<T> {
	pub fn from_elem(elem: T, n: usize) -> Self {
		let mut list = Self::new();

		match n {
			0 => list,
			1 => unsafe { list.insert_init(elem); list },
			_ => {
				unsafe { list.insert_init(elem.clone()); }
				(1..n-1).for_each(|_| unsafe { list.insert_next_nonempty(elem.clone()); });
				unsafe { list.insert_next_nonempty(elem); }
				list
			}
		}
	}
}

impl<T: ?Sized> IterList<T> {
	#[inline]
	pub const fn new() -> Self {
		Self {
			current: None,
			index:   0,
			len:     0,
			_boo:    PhantomData,
		}
	}

	/// Provides a copy of the current cursor. `O(1)`.  
	/// ```
	/// # use iterlist::*;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// let mut slice = list.as_cursor();
	///
	/// assert_eq!(slice.next(),      Some(&1));
	/// assert_eq!(slice.next(),      Some(&2));
	/// assert_eq!(slice.next(),      Some(&3));
	///
	/// assert_eq!(list.current(), Some(&1));
	/// ```
	/// # Panics
	/// Panics if the list is empty.
	pub fn as_cursor(&self) -> Cursor<'_, T> { 
		if self.len == 0 { panic!("Cannot create a cursor from an empty list"); }
		unsafe { self.as_cursor_unchecked() }
	}

	/// Provides a copy of the current cursor. `O(1)`.
	///
	/// # Safety
	/// The list must not be empty.
	pub unsafe fn as_cursor_unchecked(&self) -> Cursor<'_, T> {
		Cursor::from_raw(self.current.unwrap_unchecked().as_ptr(), self.index, self.len)
	}

	/// Split the list after the cursor. `O(1)`.  
	/// If the list is empty, has only one element, or the cursor is at the end, `None` will be returned.  
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	/// list.advance();
	/// let new_list = list.split_after().unwrap();
	///
	/// assert_eq!(format!("{:?}", list), "[1, 2]");
	/// assert_eq!(format!("{:?}", new_list), "[3]");
	/// assert_eq!(new_list.index(), 0);
	/// ```
	pub fn split_after(&mut self) -> Option<Self> {
		if self.len <= 1 { return None; }

		let c = unsafe { &mut self.current.unwrap_unchecked() };
		unsafe { c.as_ref() }.next.map(|next| {
			let mut new = Self::new();
			new.current = Some(next);

			new.len = unsafe { self.len.unchecked_sub(self.index).unchecked_sub(1) };
			self.len = unsafe { self.len.unchecked_sub(new.len) };

			unsafe { c.as_mut().next = None; }
			unsafe { new.current.unwrap_unchecked().as_mut().prev = None; }

			new
		})
	}

	/// Split the list before the cursor. `O(1)`.
	/// If the list is empty, or the cursor is at the front, `None` will be returned.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::from(vec![1, 2, 3, 4]);
	/// list.move_by(2);
	/// let new_list = list.split_before().unwrap();
	///
	/// assert_eq!(format!("{:?}", list), "[3, 4]");
	/// assert_eq!(format!("{:?}", new_list), "[1, 2]");
	/// assert_eq!(new_list.index(), 1);
	/// assert_eq!(list.index(), 0);
	/// ```
	pub fn split_before(&mut self) -> Option<Self> {
		if self.len <= 1 { return None; }

		let c = unsafe { &mut self.current.unwrap_unchecked() };
		unsafe { c.as_ref() }.prev.map(|prev| {
			let mut new = Self::new();
			new.current = Some(prev);

			new.len = self.index;
			self.len = unsafe { self.len.unchecked_sub(new.len) };
			self.index = 0;
			new.index = unsafe { new.len.unchecked_sub(1) };

			unsafe { c.as_mut().prev = None; }
			unsafe { new.current.unwrap_unchecked().as_mut().next = None; }

			new
		})
	}

	/// Move the cursor to the front of the list. `O(n)`.  
	/// Returns the number of elements traversed.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::new();
	/// list.push_next(1);
	/// list.push_next(2);
	/// list.push_next(3);
	///
	/// assert_eq!(list.current(), Some(&3));
	///
	/// let offset = list.goto_start();
	/// assert_eq!(offset, 2);
	/// assert_eq!(list.current(), Some(&1));
	/// ```
	pub fn goto_start(&mut self) -> usize {
		if self.len == 0 { return 0; }
		self.index = 0;

		for i in 0_usize.. {
			match unsafe { self.current.unwrap_unchecked().as_ref().prev } {
				Some(prev) => self.current = Some(prev),
				None => return i,
			}
		}

		unsafe { std::hint::unreachable_unchecked() }
	}

	/// Move the cursor to the back of the list. `O(n)`.  
	/// Returns the number of elements traversed.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	///
	/// let offset = list.goto_end();
	/// assert_eq!(offset, 2);
	/// assert_eq!(list.current(), Some(&3));
	/// ```
	pub fn goto_end(&mut self) -> usize {
		match self.len.checked_sub(1) {
			Some(l) => self.index = l,
			None => return 0,
		}

		for i in 0_usize.. {
			match unsafe { self.current.unwrap_unchecked().as_ref().next } {
				Some(next) => self.current = Some(next),
				None => return i,
			}
		}

		unsafe { std::hint::unreachable_unchecked() }
	}

	/// Move the cursor to the specified index. `O(n)`.
	/// If the index is out of bounds the cursor will be moved to the edge, 
	/// and `false` will be returned.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	/// list.goto(1);
	/// assert_eq!(list.current(), Some(&2));
	/// ```
	#[inline]
	#[must_use]
	pub fn goto(&mut self, index: usize) -> bool {
		match self.index.cmp(&index) {
			Ordering::Greater => !(0..self.index - index).any(|_| !self.retreat()),
			Ordering::Less    => !(0..index - self.index).any(|_| !self.advance()),
			Ordering::Equal   => true,
		}
	}

	/// Move the cursor to the specified index. `O(n)`.
	/// # Safety
	/// This function assumes that the index is within bounds.
	#[inline]
	pub unsafe fn goto_unchecked(&mut self, index: usize) {
		match self.index.cmp(&index) {
			Ordering::Greater => (0..self.index - index).for_each(|_| self.retreat_unchecked()),
			Ordering::Less    => (0..index - self.index).for_each(|_| self.advance_unchecked()),
			Ordering::Equal   => {},
		}
	}

	/// Move the cursor one step forward. `O(1)`.  
	///
	/// # Safety
	/// This function assumes that the cursor is not at the end of the list, and that the list is not empty.
	/// It is up to the caller to ensure this.
	#[inline]
	pub unsafe fn advance_unchecked(&mut self) {
		self.current = self.current.unwrap_unchecked().as_ref().next;
		self.index = self.index.unchecked_add(1);
	}

	/// Move the cursor one step forward. `O(1)`.  
	/// Returns `false` if the cursor could not be moved.
	/// ```
	/// # use iterlist::*;
	/// let mut list = list![1, 2, 3];
	/// assert_eq!(list.current(), Some(&1));
	///
	/// list.advance();
	/// assert_eq!(list.current(), Some(&2));
	/// ```
	#[inline]
	#[must_use]
	pub fn advance(&mut self) -> bool {
		if self.index+1 >= self.len { return false; }
		unsafe { self.advance_unchecked(); }
		true
	}

	/// Move the cursor n steps forward. `O(1)`.  
	/// Returns `false` if the cursor could not be moved.
	/// ```
	/// # use iterlist::*;
	/// let mut list = list![1, 2, 3];
	/// assert_eq!(list.current(), Some(&1));
	///
	/// list.advancen(2);
	/// assert_eq!(list.current(), Some(&3));
	/// ```
	#[inline]
	#[must_use]
	pub fn advancen(&mut self, n: usize) -> bool {
		if self.index + n >= self.len { return false; }
		(0..n).for_each(|_| unsafe { self.advance_unchecked() });
		true
	}

	/// Move the cursor one step backward. `O(1)`.  
	///
	/// # Safety
	/// This function assumes that the cursor is not at the end of the list, and that the list is not empty.
	/// It is up to the caller to ensure this.
	#[inline]
	pub unsafe fn retreat_unchecked(&mut self) {
		self.current = self.current.unwrap_unchecked().as_ref().prev;
		self.index = self.index.unchecked_sub(1);
	}

	/// Move the cursor one step backward. `O(1)`.  
	/// Returns `false` if the cursor could not be moved.
	/// ```
	/// # use iterlist::*;
	/// let mut list = list![1, 2, 3];
	///
	/// list.goto_end();
	/// assert_eq!(list.current(), Some(&3));
	///
	/// list.retreat();
	/// assert_eq!(list.current(), Some(&2));
	/// ```
	#[inline]
	#[must_use]
	pub fn retreat(&mut self) -> bool {
		if self.index == 0 { return false; }
		unsafe { self.retreat_unchecked(); }
		true
	}

	/// Move the cursor n steps backward. `O(1)`.
	/// Returns `false` if the cursor could not be moved.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	/// list.goto_end();
	/// assert_eq!(list.current(), Some(&3));
	///
	/// list.retreatn(2);
	/// assert_eq!(list.current(), Some(&1));
	/// ```
	#[inline]
	#[must_use]
	pub fn retreatn(&mut self, n: usize) -> bool {
		if n > self.index { return false; }
		(0..n).for_each(|_| unsafe { self.retreat_unchecked() });
		true
	}

	/// Move the cursor by a given offset. `O(n)`.  
	/// If the offset is out of bounds the cursor will be moved to the edge, 
	/// and `false` will be returned.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	///
	/// list.goto_end();
	/// assert_eq!(list.index(), 2);
	///
	/// list.move_by(-2);
	/// assert_eq!(list.index(), 0);
	/// assert_eq!(list.current(), Some(&1));
	///
	/// assert!(!list.move_by(10));
	/// assert_eq!(list.index(), 2);
	/// ```
	#[inline]
	#[must_use]
	pub fn move_by(&mut self, offset: isize) -> bool {
		match offset.cmp(&0) {
			Ordering::Greater => (0..offset ).fold(true, |_, _| self.advance()),
			Ordering::Less    => (0..-offset).fold(true, |_, _| self.retreat()),
			Ordering::Equal   => true,
		}
	}

	fn get_raw(&self, offset: isize) -> Option<NonNull<Node<T>>> {
		if self.index as isize + offset < 0 
			|| self.index as isize + offset >= self.len as isize 
			{ return None; }

		match offset.cmp(&0) {
			Ordering::Greater => (0.. offset).try_fold(unsafe { self.current.unwrap_unchecked() }, |ptr, _| unsafe { ptr.as_ref() }.next),
			Ordering::Less    => (0..-offset).try_fold(unsafe { self.current.unwrap_unchecked() }, |ptr, _| unsafe { ptr.as_ref() }.prev),
			Ordering::Equal   => self.current
		}
	}

	/// Apply the current state of a cursor to the list. `O(n)`.
	///
	/// # Safety
	/// The cursor must be valid and point to a `Node` in the same list.
	#[inline]
	pub unsafe fn apply_cursor_unchecked(&mut self, cursor: &Cursor<T>) {
		self.current = cursor.current_raw();
		self.index   = cursor.index();
	}

	/// Get a ref to an element at the given offset from the cursor. `O(n)`.
	/// Returns `None` if the offset is out of bounds.
	/// ```
	/// # use iterlist::*;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// assert_eq!(list.current(), Some(&1));
	/// assert_eq!(list.get(1), Some(&2));
	/// assert_eq!(list.get(-1), None);
	/// ```
	#[inline]
	pub fn get(&self, offset: isize) -> Option<&T> {
		self.get_raw(offset).map(|ptr| unsafe { &ptr.as_ref().elem })
	}

	/// Get a ref to the current element. `O(1)`.
	/// # Safety
	/// This function assumes that the list is not empty.
	#[inline]
	pub unsafe fn current_unchecked(&self) -> &T {
		&self.current.unwrap_unchecked().as_ref().elem
	}

	/// Get a mut ref to an element at the given offset. `O(n)`.
	/// Returns `None` if the offset is out of bounds.  
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	///
	/// *list.get_mut(1).unwrap() = 4;
	/// assert_eq!(format!("{:?}", list), "[1, 4, 3]");
	/// ```
	#[inline]
	pub fn get_mut(&mut self, offset: isize) -> Option<&mut T> {
		self.get_raw(offset).map(|ref mut ptr| unsafe { &mut ptr.as_mut().elem })
	}

	/// Get a mut ref to the current element. `O(1)`.
	/// # Safety
	/// This function assumes that the list is not empty.
	#[inline]
	pub unsafe fn get_current_mut_unchecked(&mut self) -> &mut T {
		&mut self.current.unwrap_unchecked().as_mut().elem
	}

	/// Get a ref to the current element. `O(1)`.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	///
	/// assert_eq!(list.current(), Some(&1));
	/// ```
	#[inline]
	pub fn current(&self) -> Option<&T> {
		self.current.map(|c| unsafe { &c.as_ref().elem })
	}

	/// Get a mut ref to the current element. `O(1)`.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	///
	/// *list.get_current_mut().unwrap() = 4;
	/// assert_eq!(format!("{:?}", list), "[4, 2, 3]");
	/// ```
	#[inline]
	pub fn get_current_mut(&mut self) -> Option<&mut T> {
		self.current.map(|ref mut c| unsafe { &mut c.as_mut().elem })
	}

	/// Get the number of elements in the list. `O(1)`.
	/// ```
	/// # use iterlist::*;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// assert_eq!(list.len(), 3);
	/// ```
	#[inline]
	pub const fn len(&self) -> usize {
		self.len
	}

	/// Check if the list is empty. `O(1)`.
	/// ```
	/// # use iterlist::*;
	/// let list: IterList<u8> = IterList::new();
	/// assert!(list.is_empty());
	/// ```
	#[inline]
	pub const fn is_empty(&self) -> bool {
		self.len == 0
	}

	/// Get the index of the cursor `O(1)`.
	/// ```
	/// # use iterlist::*;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// assert_eq!(list.index(), 0);
	/// ```
	#[inline]
	pub const fn index(&self) -> usize {
		self.index
	}

	pub unsafe fn insert_next_nonempty_raw(&mut self, mut new: NonNull<Node<T>>) {
		let c = unsafe { &mut self.current.unwrap_unchecked() };

		unsafe {
			if let Some(mut next) = c.as_mut().next {
				next.as_mut().prev = Some(new);
				new.as_mut().next  = Some(next);
			}

			c.as_mut().next   = Some(new);
			new.as_mut().prev = Some(*c);
		}

		self.len = unsafe { self.len.unchecked_add(1) };
	}

	pub unsafe fn insert_prev_nonempty_raw(&mut self, mut new: NonNull<Node<T>>) {
		let c = unsafe { &mut self.current.unwrap_unchecked() };

		unsafe {
			if let Some(mut prev) = c.as_mut().prev {
				prev.as_mut().next = Some(new);
				new.as_mut().prev  = Some(prev);
			}

			c.as_mut().prev   = Some(new);
			new.as_mut().next = Some(*c);
		}

		self.index = unsafe { self.index.unchecked_add(1) };
		self.len = unsafe { self.len.unchecked_add(1) };
	}

	/// Insert an element into the list. `O(1)`.
	///
	/// # Safety
	/// This list must be empty.
	#[inline]
	pub unsafe fn insert_init_raw(&mut self, new: NonNull<Node<T>>) {
		self.current = Some(new);
		self.len = 1;
	}
}

impl<T: Clone> IterList<[T]> {
	/// Insert an element into the list. `O(1)`.
	///
	///```
	///# use iterlist::*;
	///let mut list: IterList<[u8]> = IterList::new();
	///unsafe { list.insert_init_cpy(&[]) };
	/// assert_eq!(list.current(), Some(&[][..]));
	///```
	/// # Safety
	/// This list must be empty.
	#[inline]
	pub unsafe fn insert_init_cpy(&mut self, elem: &[T]) {
		self.insert_init_raw(Node::new_nonnull(elem));
	}

	/// Insert an element after the cursor, retaining current position. `O(1)`.  
	/// If the list is empty it will be inserted at index 0.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::new();
	/// list.insert_next_cpy(&[1]);
	/// list.insert_next_cpy(&[2, 3]);
	/// list.insert_next_cpy(&[4]);
	/// 
	/// assert_eq!(list.current(), Some(&[1][..]));
	/// ```
	#[inline]
	pub fn insert_next_cpy(&mut self, elem: &[T]) {
		match self.current {
			None    => unsafe { self.insert_init_cpy(elem) },
			Some(_) => unsafe { self.insert_next_nonempty_cpy(elem) },
		}
	}

	/// Insert an element after the cursor, retaining current position. `O(1)`.
	///
	/// # Safety
	/// This list must not be empty.
	#[inline]
	pub unsafe fn insert_next_nonempty_cpy(&mut self, elem: &[T]) {
		self.insert_next_nonempty_raw(Node::new_nonnull(elem))
	}

	/// Insert an element before the cursor, retaining current position. `O(1)`.  
	/// If the list is empty it will be inserted at index 0.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::new();
	/// list.insert_prev_cpy(&[1]);
	/// list.insert_prev_cpy(&[2, 3]);
	/// list.insert_prev_cpy(&[4]);
	/// 
	/// assert_eq!(list.current(), Some(&[1][..]));
	/// ```
	#[inline]
	pub fn insert_prev_cpy(&mut self, elem: &[T]) {
		match self.current {
			None    => unsafe { self.insert_init_cpy(elem) },
			Some(_) => unsafe { self.insert_prev_nonempty_cpy(elem) },
		}
	}

	/// Insert an element before the cursor, retaining current position. `O(1)`.
	///
	/// # Safety
	/// This list must not be empty.
	#[inline]
	pub unsafe fn insert_prev_nonempty_cpy(&mut self, elem: &[T]) {
		self.insert_prev_nonempty_raw(Node::new_nonnull(elem))
	}

	/// Push an element after the cursor, moving the cursor to it. `O(1)`.  
	/// If the list is empty it will be inserted at index 0.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::new();
	/// list.push_next_cpy(&[1]);
	/// list.push_next_cpy(&[2, 3]);
	/// list.push_next_cpy(&[4]);
	///
	/// assert_eq!(list.current(), Some(&[4][..]));
	/// ```
	#[inline]
	pub fn push_next_cpy(&mut self, elem: &[T]) {
		match self.current {
			None => unsafe { self.insert_init_cpy(elem) },
			Some(_) => {
				self.insert_next_cpy(elem);
				let _ = self.advance(); }
		}
	}

	/// Push an element before the cursor, moving the cursor to it. `O(1)`.  
	/// If the list is empty it will be inserted at index 0.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::new();
	/// list.push_prev_cpy(&[1]);
	/// list.push_prev_cpy(&[2, 3]);
	/// list.push_prev_cpy(&[4]);
	///
	/// assert_eq!(list.current(), Some(&[4][..]));
	/// ```
	#[inline]
	pub fn push_prev_cpy(&mut self, elem: &[T]) {
		match self.current {
			None => unsafe { self.insert_init_cpy(elem) },
			Some(_) => {
				self.insert_prev_cpy(elem);
				let _ = self.retreat(); }
		}
	}
}

impl<T: Sized> IterList<T> {
	/// Create a new list with N zeroed elements. `O(n)`.  
	/// *Secret Pro Tip:*  
	/// This can potentially be a roundabout way to get a sort-of `with_capacity` method.
	/// Although using it in such a way will rarely improve performance (iterlist never reallocates unlike `Vec`),
	/// and can actually hurt it in most cases. That should only be done when
	/// you have some very timing sensitive code, and you cant afford to have to allocate memory.  
	/// ie.. never.
	///
	/// # Safety
	/// Type `T` must be safe to initialize as zeroed.
	pub unsafe fn new_zeroed(count: usize) -> Self {
		(0..count).fold(Self::new(), |mut list, _| {
			list.insert_next(std::mem::MaybeUninit::zeroed().assume_init()); list
		})
	}

	/// Insert an element into the list. `O(1)`.
	///
	/// # Safety
	/// This list must be empty.
	#[inline]
	pub unsafe fn insert_init(&mut self, elem: T) {
		self.insert_init_raw(Node::new_nonnull(elem))
	}

	/// Insert an element after the cursor, retaining current position. `O(1)`.  
	/// If the list is empty it will be inserted at index 0.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::new();
	/// list.insert_next(1);
	/// list.insert_next(2);
	/// list.insert_next(3);
	/// 
	/// assert_eq!(list.current(), Some(&1));
	/// assert_eq!(&format!("{:?}", list), "[1, 3, 2]");
	/// ```
	#[inline]
	pub fn insert_next(&mut self, elem: T) {
		match self.current {
			None    => unsafe { self.insert_init(elem) },
			Some(_) => unsafe { self.insert_next_nonempty(elem) },
		}
	}

	/// Insert an element after the cursor, retaining current position. `O(1)`.
	///
	/// # Safety
	/// This list must not be empty.
	#[inline]
	pub unsafe fn insert_next_nonempty(&mut self, elem: T) {
		self.insert_next_nonempty_raw(Node::new_nonnull(elem))
	}

	/// Insert an element before the cursor, retaining current position. `O(1)`.  
	/// If the list is empty it will be inserted at index 0.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::new();
	/// list.insert_prev(1);
	/// list.insert_prev(2);
	/// list.insert_prev(3);
	///
	/// assert_eq!(list.current(), Some(&1));
	/// assert_eq!(&format!("{:?}", list), "[2, 3, 1]");
	/// ```
	#[inline]
	pub fn insert_prev(&mut self, elem: T) {
		match self.current {
			None    => unsafe { self.insert_init(elem) },
			Some(_) => unsafe { self.insert_prev_nonempty(elem) },
		}
	}

	/// Insert an element before the cursor, retaining current position. `O(1)`.
	///
	/// # Safety
	/// This list must not be empty.
	#[inline]
	pub unsafe fn insert_prev_nonempty(&mut self, elem: T) {
		self.insert_prev_nonempty_raw(Node::new_nonnull(elem))
	}

	/// Push an element after the cursor, moving the cursor to it. `O(1)`.  
	/// If the list is empty it will be inserted at index 0.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::new();
	/// list.push_next(1);
	/// list.push_next(2);
	/// list.push_next(3);
	///
	/// assert_eq!(list.current(), Some(&3));
	/// assert_eq!(format!("{:?}", list), "[1, 2, 3]");
	/// ```
	#[inline]
	pub fn push_next(&mut self, elem: T) {
		match self.current {
			None => unsafe { self.insert_init(elem) },
			Some(_) => {
				self.insert_next(elem);
				let _ = self.advance(); }
		}
	}

	/// Push an element before the cursor, moving the cursor to it. `O(1)`.  
	/// If the list is empty it will be inserted at index 0.
	/// ```
	/// # use iterlist::*;
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
		match self.current {
			None => unsafe { self.insert_init(elem) },
			Some(_) => {
				self.insert_prev(elem);
				let _ = self.retreat(); }
		}
	}

	/// Remove the current element and return it. `O(1)`.  
	/// The cursor will then point to the next element (perserving current index).  
	/// If the removed element was at the end of the list, the cursor will point to the previous
	/// element and `false` will be returned.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	/// assert_eq!(list.consume(), Some((1, true)));
	/// assert_eq!(&format!("{:?}", list), "[2, 3]");
	/// assert_eq!(list.current(), Some(&2));
	/// ```
	pub fn consume(&mut self) -> Option<(T, bool)> {
		let node = unsafe { Box::from_raw(self.current?.as_ptr()) };
		self.len = unsafe { self.len.unchecked_sub(1) };

		node.prev.map(|mut prev| unsafe { prev.as_mut() }.next = node.next);

		match node.next {
			Some(mut next) => {
				unsafe { next.as_mut() }.prev = node.prev;
				self.current = Some(next);
				Some((node.elem, true))
			},
			None => {
				self.current = node.prev;
				// if this happens on the only element we dont wanna decrement index
				self.index = self.index.saturating_sub(1); 
				Some((node.elem, false))
			}
		}
	}

	/// Remove the current element and return it. `O(1)`.  
	/// The cursor will then point to the previous element.  
	/// If the removed element was at the start of the list, the cursor will point to the next
	/// element and `false` will be returned.
	/// ```
	/// # use iterlist::*;
	/// let mut list = list![1, 2, 3];
	/// list.move_by(1);
	/// assert_eq!(list.consume_back(), Some((2, true)));
	/// assert_eq!(&format!("{:?}", list), "[1, 3]");
	/// assert_eq!(list.current(), Some(&1));
	/// ```
	pub fn consume_back(&mut self) -> Option<(T, bool)> {
		let node = unsafe { Box::from_raw(self.current?.as_ptr()) };
		self.len = unsafe { self.len.unchecked_sub(1) };
		node.next.map(|mut next| unsafe { next.as_mut() }.prev = node.prev);

		match node.prev {
			Some(mut prev) => {
				unsafe { prev.as_mut() }.next = node.next;
				self.current = Some(prev);
				self.index = unsafe { self.index.unchecked_sub(1) };
				Some((node.elem, true))
			},
			None => {
				self.current = node.next;
				Some((node.elem, false))
			}
		}
	}

	/// Replace the current element with a new one. `O(1)`.  
	/// Returns the old element.  
	/// If the list is empty, the new element will be inserted, and `None` returned.
	/// ```
	/// # use iterlist::*;
	/// let mut list = list![1, 2, 3];
	/// assert_eq!(list.replace_cursor(4), Some(1));
	/// assert_eq!(format!("{:?}", list), "[4, 2, 3]");
	/// ```
	#[inline]
	pub fn replace_cursor(&mut self, elem: T) -> Option<T> {
		match self.current {
			None    => { self.push_next(elem); None },
			Some(ref mut c) => Some(std::mem::replace(unsafe { &mut c.as_mut().elem }, elem)),
		}
	}

	/// Remove an element at the given index. `O(n)`.
	/// If the index is out of bounds, `None` will be returned.
	/// ```
	/// # use iterlist::*;
	/// let mut list = list![1, 2, 3];
	/// assert_eq!(list.remove(1), Some(2));
	/// assert_eq!(format!("{:?}", list), "[1, 3]");
	/// ```
	pub fn remove(&mut self, index: usize) -> Option<T> {
		if index >= self.len { return None; }

		let mut c = self.current?;

		match self.index.cmp(&index) {
			Ordering::Greater => (0..self.index - index).try_for_each(|_| { c = unsafe { c.as_ref() }.prev?; Some(()) })?,
			Ordering::Less    => (0..index - self.index).try_for_each(|_| { c = unsafe { c.as_ref() }.next?; Some(()) })?,
			Ordering::Equal   => {},
		}

		let node = unsafe { Box::from_raw(c.as_ptr()) };
		self.len = unsafe { self.len.unchecked_sub(1) };

		node.prev.map(|mut prev| unsafe { prev.as_mut() }.next = node.next);

		match unsafe { self.current.unwrap_unchecked().as_ref().next } {
			Some(mut next) => {
				unsafe { next.as_mut() }.prev = node.prev;
				self.current = Some(next);
			},
			None => {
				self.current = unsafe { self.current.unwrap_unchecked().as_ref().prev };
				self.index = unsafe { self.index.unchecked_sub(1) };
			}
		}

		Some(node.elem)
	}
}

impl<T: ?Sized> Drop for IterList<T> {
	/// Drop the list. `O(n)`.
	fn drop(&mut self) {
		// optimizing the direction we drop
		if self.index < self.len - self.index {
			self.goto_start();

			let Some(mut current) = self.current else { return; };

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

		let Some(mut current) = self.current else { return; };

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

impl<T> std::ops::Index<isize> for IterList<T> {
	type Output = T;

	/// Essentially equivalent to `get`. `O(n)`.  
	/// # Panics
	/// Panics if the index is out of bounds.
	#[inline]
	fn index(&self, index: isize) -> &Self::Output {
		self.get(index).unwrap_or_else(|| panic!("Index out of bounds"))
	}
}

impl<T> std::ops::IndexMut<isize> for IterList<T> {
	#[inline]
	fn index_mut(&mut self, index: isize) -> &mut Self::Output {
		self.get_mut(index).unwrap_or_else(|| panic!("Index out of bounds"))
	}
}

impl<T: Clone> Clone for IterList<T> {
	/// Clone the list. `O(n)`.  
	/// Cursor position is retained.
	/// ```
	/// # use iterlist::*;
	/// let list = vec![1, 2, 3].into_iter().collect::<IterList<_>>();
	/// assert_eq!(format!("{:?}", list), "[1, 2, 3]");
	///
	/// let cloned = list.clone();
	/// assert_eq!(format!("{:?}", cloned), "[1, 2, 3]");
	/// ```
	fn clone(&self) -> Self {
		let mut new = IterList::new();
		if self.len == 0 { return new; }

		let mut cursor = unsafe { self.as_cursor_unchecked() };

		// direction we clone has no effect on performance since we
		// need to go back to the original position anyway
		cursor.goto_start();
		unsafe { new.insert_init(cursor.current_unchecked().clone()); }

		while cursor.advance() {
			new.push_next(unsafe { cursor.current_unchecked().clone() });
		}

		unsafe { new.goto_unchecked(self.index); }
		new
	}
}

impl<T> Iterator for IterList<T> {
	type Item = T;

	/// An iterator consuming elements from the list. `O(1)`.  
	/// This works exactly like `consume_forward` and is based on the cursor position.
	/// ```
	/// # use iterlist::*;
	/// let list = IterList::from(vec![1, 2, 3]);
	///
	/// // list moved
	/// let num = list.fold(0, |acc, elem| acc + elem);
	/// assert_eq!(num, 6);
	/// ```
	#[inline]
	fn next(&mut self) -> Option<Self::Item> {
		self.consume().map(|(e, _)| e)
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) 
		{ (self.len - self.index, Some(self.len)) }
}

impl<T> DoubleEndedIterator for IterList<T> {
	/// An iterator consuming elements from the list. `O(1)`.  
	/// This works exactly like `consume_backward` and is based on the cursor position.
	/// ```
	/// # use iterlist::*;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	///
	/// list.goto_end();
	///
	/// let two = list.nth_back(1);
	/// assert_eq!(two, Some(2));
	/// ```
	#[inline]
	fn next_back(&mut self) -> Option<Self::Item> {
		self.consume_back().map(|(e, _)| e)
	}
}

impl<T> ExactSizeIterator for IterList<T> { }

impl<T> From<Vec<T>> for IterList<T> {
	/// Create a new list from a Vec. `O(n)`.  
	/// Cursor is set to the front of the list.
	/// ```
	/// # use iterlist::*;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// assert_eq!(format!("{:?}", list), "[1, 2, 3]");
	/// assert_eq!(list.current(), Some(&1));
	/// ```
	fn from(mut vec: Vec<T>) -> Self {
		if vec.is_empty() { return Self::new(); }

		let mut new = Self::new();
		unsafe { new.insert_init(vec.pop().unwrap_unchecked()); }
		while let Some(elem) = vec.pop() { 
			unsafe { new.insert_prev_nonempty(elem); }
			unsafe { new.retreat_unchecked(); }
		}
		new
	}
}

impl<T> FromIterator<T> for IterList<T> {
	/// Create a new list from an iterator. `O(n)`.  
	/// Cursor is set to the front of the list.
	/// ```
	/// # use iterlist::*;
	/// let list = (1..=3).into_iter().collect::<IterList<_>>();
	/// assert_eq!(format!("{:?}", list), "[1, 2, 3]");
	/// assert_eq!(list.current(), Some(&1));
	/// ```
	fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
		let mut iter = iter.into_iter();
		let mut new = Self::new();

		if let Some(elem) = iter.next() 
			{ unsafe { new.insert_init(elem); } }
		else { return new; }

		let origin = new.current;

		iter.for_each(|elem| {
			unsafe { new.insert_next_nonempty(elem); }
			unsafe { new.advance_unchecked(); }
		});

		new.index = 0;
		new.current = origin;
		new
	}
}

impl<T: fmt::Debug> fmt::Debug for IterList<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "{:?}", self.as_cursor())
	}
}
