use std::cmp::Ordering;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::fmt;

use crate::node::Node;

pub struct Cursor<'l, T: ?Sized> {
	current: Option<NonNull<Node<T>>>,
	index:   usize,
	len:     usize,
	_boo:    PhantomData<&'l super::IterList<T>>,
}

impl<T: ?Sized> Default for Cursor<'_, T> {
	fn default() -> Self {
		Self {
			current: None,
			index:   0,
			len:     0,
			_boo:    PhantomData,
		}
	}
}

impl<T: Sized> Cursor<'_, T> {
	pub fn dangling() -> Self {
		Self {
			current: Some(NonNull::dangling()),
			.. Self::default()
		}
	}
}

impl<T: ?Sized> Cursor<'_, T> {
	/// Create a new cursor from raw parts. `O(1)`.
	/// # Safety
	/// The caller must ensure that the ptr, index and len are valid.
	pub unsafe fn from_raw(ptr: *mut Node<T>, index: usize, len: usize) -> Self {
		Self {
			current: Some(NonNull::new_unchecked(ptr)),
			index,
			len,
			_boo: PhantomData,
		}
	}

	/// Pretty self explanatory. `O(1)`.
	/// # Safety
	/// This is generally not the intended usage of `Cursor`, it's not meant to live longer than the list.  
	/// The caller must ensure that the cursor is not used after the list is dropped to avoid a use
	/// after free.
	pub unsafe fn clone(&self) -> Self {
		Self {
			current: self.current,
			index:   self.index,
			len:     self.len,
			_boo:    PhantomData,
		}
	}

	/// Move the cursor to the front of the list. `O(n)`.  
	/// Returns the number of elements traversed.
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
	#[inline]
	#[must_use]
	pub fn advance(&mut self) -> bool {
		if self.index+1 >= self.len { return false; }
		unsafe { self.advance_unchecked(); }
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
	#[inline]
	#[must_use]
	pub fn retreat(&mut self) -> bool {
		if self.index == 0 { return false; }
		unsafe { self.retreat_unchecked(); }
		true
	}

	/// Move the cursor by a given offset. `O(n)`.  
	/// If the offset is out of bounds the cursor will be moved to the edge, 
	/// and `false` will be returned.
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
	#[inline]
	pub fn get(&self, offset: isize) -> Option<&T> {
		self.get_raw(offset).map(|ptr| unsafe { &ptr.as_ref().elem })
	}

	pub fn current_raw(&self) -> Option<NonNull<Node<T>>> {
		self.current
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
	#[inline]
	pub fn current(&self) -> Option<&T> {
		self.current.map(|c| unsafe { &c.as_ref().elem })
	}

	/// Get a mut ref to the current element. `O(1)`.
	#[inline]
	pub fn get_current_mut(&mut self) -> Option<&mut T> {
		self.current.map(|ref mut c| unsafe { &mut c.as_mut().elem })
	}

	/// Get the number of elements in the list. `O(1)`.
	/// ```
	/// # use iterlist::IterList;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// assert_eq!(list.len(), 3);
	/// ```
	#[inline]
	pub const fn len(&self) -> usize {
		self.len
	}

	/// Check if the list is empty. `O(1)`.
	/// ```
	/// # use iterlist::IterList;
	/// let list: IterList<u8> = IterList::new();
	/// assert!(list.is_empty());
	/// ```
	#[inline]
	pub const fn is_empty(&self) -> bool {
		self.len == 0
	}

	/// Get the index of the cursor `O(1)`.
	/// ```
	/// # use iterlist::IterList;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// assert_eq!(list.index(), 0);
	/// ```
	#[inline]
	pub const fn index(&self) -> usize {
		self.index
	}
}


impl<T> std::ops::Index<isize> for Cursor<'_, T> {
	type Output = T;

	/// Essentially equivalent to `get`. `O(n)`.  
	/// # Panics
	/// Panics if the index is out of bounds.
	#[inline]
	fn index(&self, index: isize) -> &Self::Output {
		self.get(index).unwrap_or_else(|| panic!("Index out of bounds"))
	}
}

impl<'l, T> Iterator for Cursor<'l, T> {
	type Item = &'l T;

	#[inline]
	fn next(&mut self) -> Option<Self::Item> {
		self.current.map(|c| {
			self.current = unsafe { c.as_ref().next };
			self.index += 1;
			unsafe { &c.as_ref().elem }
		})
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) 
		{ (self.len - self.index, Some(self.len)) }
}

impl<T> DoubleEndedIterator for Cursor<'_, T> {
	#[inline]
	fn next_back(&mut self) -> Option<Self::Item> {
		self.current.map(|c| {
			self.current = unsafe { c.as_ref().prev };
			self.index -= 1;
			unsafe { &c.as_ref().elem }
		})
	}
}

impl<T: fmt::Debug> fmt::Debug for Cursor<'_, T> {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "[")?;
		if self.is_empty() { return write!(f, "]"); }

		let mut cursor = unsafe { self.clone() };
		cursor.goto_start();

		write!(f, "{:?}", unsafe { cursor.current_unchecked() })?;
		cursor.skip(1).try_for_each(|e| write!(f, ", {e:?}"))?;

		write!(f, "]")
	}
}
