use std::fmt::Debug;
use std::marker::PhantomData;
use std::cmp::Ordering;
use std::mem;
use std::ptr::NonNull;
use std::ops::Not;

/// A doubly linked list. The `IterList` object is a fat pointer of a `Cursor + length`, which owns the underlying data.  
/// This means the total stack size is 3 words; each element is 2 words + element size.
pub struct IterList<T> {
	current: NonNull<Node<T>>,
	index:   usize,
	len:     usize,
	_boo:    PhantomData<T>,
}

struct Node<T> {
	next: Option<NonNull<Node<T>>>,
	prev: Option<NonNull<Node<T>>>,
	elem: T,
}

impl<T> Node<T> {
	fn new_nonnull(elem: T) -> NonNull<Self> {
		unsafe {
			NonNull::new_unchecked(Box::into_raw(Box::new(Self {
				next: None,
				prev: None,
				elem,
			})))
		}
	}
}

unsafe impl<T: Send> Send for IterList<T> {}
unsafe impl<T: Sync> Sync for IterList<T> {}

impl<T> IterList<T> {
	/// Create a new empty list. `O(1)`.  
	/// Does not allocate any memory.
	/// ```
	/// # use iterlist::IterList;
	/// let list: IterList<u8> = IterList::new();
	/// assert_eq!(list.len(), 0);
	/// ```
	#[inline]
	pub const fn new() -> Self {
		Self { 
			current: NonNull::dangling(),
			len:     0,
			index:   0,
			_boo:    PhantomData
		}
	}

	/// Create a new list with N zeroed elements. `O(n)`.  
	/// *Secret Pro Tip:*  
	/// This can potentially be a roundabout way to get a sort-of `with_capacity` method.
	/// Although using it in such a way will rarely improve performance (iterlist never reallocates unlike `Vec`),
	/// and can actually hurt it in most cases. That should only be done when
	/// you have some very timing sensitive code, and you cant afford to have to allocate memory.
	///
	/// # Safety
	/// Type `T` must be safe to initialize as zeroed.
	///
	/// ```
	/// # use iterlist::IterList;
	/// let list: IterList<u8> = unsafe { IterList::new_zeroed(3) };
	/// assert_eq!(list.len(), 3);
	/// assert_eq!(format!("{list:?}"), "[0, 0, 0]");
	/// ```
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

		match self.len {
			0 => self.current = new,
			_ => {
				unsafe {
					if let Some(mut next) = self.current.as_mut().next {
						next.as_mut().prev = Some(new);
						new.as_mut().next  = Some(next);
					}

					self.current.as_mut().next = Some(new);
					new.as_mut().prev          = Some(self.current);
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

		match self.len {
			0 => self.current = new,
			_ => {
				unsafe {
					if let Some(mut prev) = self.current.as_mut().prev {
						prev.as_mut().next = Some(new);
						new.as_mut().prev  = Some(prev);
					}

					self.current.as_mut().prev = Some(new);
					new.as_mut().next          = Some(self.current);
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
	pub fn push_next(&mut self, elem: T) {
		if self.len == 0 {
			self.insert_next(elem);
			return;
		}

		self.insert_next(elem);
		let _ = self.advance();
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
	pub fn push_prev(&mut self, elem: T) {
		if self.len == 0 {
			self.insert_prev(elem);
			return;
		}

		self.insert_prev(elem);
		let _ = self.retreat();
	}

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
	pub fn move_to_front(&mut self) -> usize {
		if self.len == 0 { return 0; }

		self.index = 0;

		for i in 0_usize.. {
			match unsafe { self.current.as_ref().prev } {
				Some(prev) => self.current = prev,
				None => return i,
			}
		}
		unsafe { std::hint::unreachable_unchecked() }
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
	pub fn move_to_back(&mut self) -> usize {
		if self.len == 0 { return 0; }

		self.index = self.len - 1;

		for i in 0_usize.. {
			match unsafe { self.current.as_ref().next } {
				Some(next) => self.current = next,
				None => return i,
			}
		}
		unsafe { std::hint::unreachable_unchecked() }
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
		if self.len == 0 { return false; }

		unsafe { self.current.as_ref() }.next.map(|next| {
				self.current = next;
				self.index += 1; })
			.is_some()
	}

	/// Move the cursor one step forward. `O(1)`.  
	///
	/// # Safety
	/// This function assumes that the cursor is not at the end of the list,
	/// it is up to the caller to ensure this.
	#[inline]
	pub unsafe fn advance_unchecked(&mut self) {
		self.current = self.current.as_ref().next.unwrap_unchecked();
		self.index += 1;
	}

	/// Move the cursor one step backward. `O(1)`.  
	/// Returns `false` if the cursor could not be moved.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	///
	/// list.move_to_back();
	/// assert_eq!(list.current(), Some(&3));
	///
	/// list.retreat();
	/// assert_eq!(list.current(), Some(&2));
	/// ```
	#[inline]
	#[must_use]
	pub fn retreat(&mut self) -> bool {
		if self.len == 0 { return false; }

		unsafe { self.current.as_ref() }.prev.map(|prev| {
				self.current = prev;
				self.index -= 1; })
			.is_some()
	}

	/// Move the cursor one step backward. `O(1)`.  
	///
	/// # Safety
	/// This function assumes that the cursor is not at the end of the list,
	/// it is up to the caller to ensure this.
	#[inline]
	pub unsafe fn retreat_unchecked(&mut self) {
		self.current = self.current.as_ref().prev.unwrap_unchecked();
		self.index -= 1;
	}

	/// Move the cursor by a given offset. `O(n)`.  
	/// If the offset is out of bounds the cursor will be moved to the edge, 
	/// and `false` will be returned.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	///
	/// list.move_to_back();
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
		if self.index as isize + offset < 0 || self.index as isize + offset >= self.len as isize {
			return None;
		}

		match offset.cmp(&0) {
			Ordering::Greater => (0.. offset).try_fold(self.current, |mut ptr, _| unsafe { ptr.as_ref() }.next.map(|c| { ptr = c; ptr })),
			Ordering::Less    => (0..-offset).try_fold(self.current, |mut ptr, _| unsafe { ptr.as_ref() }.prev.map(|c| { ptr = c; ptr })),
			Ordering::Equal   => Some(self.current)
		}
	}

	/// Get a ref to an element at the given offset from the cursor. `O(n)`.
	/// Returns `None` if the offset is out of bounds.
	/// ```
	/// # use iterlist::IterList;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// assert_eq!(list.current(), Some(&1));
	/// assert_eq!(list.get(1), Some(&2));
	/// assert_eq!(list.get(-1), None);
	/// ```
	#[inline]
	pub fn get(&self, offset: isize) -> Option<&T> {
		self.get_raw(offset).map(|ptr| unsafe { &ptr.as_ref().elem })
	}

	/// Get a mut ref to an element at the given offset. `O(n)`.
	/// Returns `None` if the offset is out of bounds.  
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	///
	/// *list.get_mut(1).unwrap() = 4;
	/// assert_eq!(format!("{:?}", list), "[1, 4, 3]");
	/// ```
	#[inline]
	pub fn get_mut(&mut self, offset: isize) -> Option<&mut T> {
		self.get_raw(offset).map(|mut ptr| unsafe { &mut ptr.as_mut().elem })
	}

	/// Remove the current element and return it. `O(1)`.  
	/// The cursor will then point to the next element.  
	/// If the removed element was at the end of the list, the cursor will point to the previous
	/// element and `false` will be returned.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	/// assert_eq!(list.consume_forward(), Some((1, true)));
	/// assert_eq!(&format!("{:?}", list), "[2, 3]");
	/// assert_eq!(list.current(), Some(&2));
	/// ```
	pub fn consume_forward(&mut self) -> Option<(T, bool)> {
		if self.len == 0 { return None; }

		let node = unsafe { Box::from_raw(self.current.as_ptr()) };

		self.len -= 1;
		node.prev.map(|mut prev| unsafe { prev.as_mut().next = node.next });

		match node.next {
			Some(mut next) => {
				unsafe { next.as_mut().prev = node.prev; }
				self.current = next;
				Some((node.elem, true))
			},
			None => {
				self.current = node.prev.unwrap_or(NonNull::dangling()); // FIXME: idkk if this works
				Some((node.elem, false))
			}
		}
	}

	/// Remove the current element and return it. `O(1)`.  
	/// The cursor will then point to the previous element.  
	/// If the removed element was at the end of the list, the cursor will point to the previous
	/// element and `false` will be returned.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	/// list.move_by(1);
	/// assert_eq!(list.consume_backward(), Some((2, true)));
	/// assert_eq!(&format!("{:?}", list), "[1, 3]");
	/// assert_eq!(list.current(), Some(&1));
	/// ```
	pub fn consume_backward(&mut self) -> Option<(T, bool)> {
		if self.len == 0 { return None; }

		let node = unsafe { Box::from_raw(self.current.as_ptr()) };

		self.len -= 1;
		self.index -= 1;
		node.next.map(|mut next| unsafe { next.as_mut().prev = node.prev });

		match node.prev {
			Some(mut prev) => {
				unsafe { prev.as_mut().next = node.next; }
				self.current = prev;
				Some((node.elem, true))
			},
			None => {
				self.current = node.next.unwrap_or(NonNull::dangling());
				Some((node.elem, false))
			}
		}
	}

	/// Replace the current element with a new one. `O(1)`.  
	/// Returns the old element.  
	/// If the list is empty, the new element will be inserted, and `None` returned.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	/// assert_eq!(list.replace_cursor(4), Some(1));
	/// assert_eq!(format!("{:?}", list), "[4, 2, 3]");
	/// ```
	#[inline]
	pub fn replace_cursor(&mut self, elem: T) -> Option<T> {
		match self.len {
			0 => { self.push_next(elem); None },
			_ => Some(std::mem::replace(unsafe { &mut self.current.as_mut().elem }, elem)),
		}
	}

	/// Apply the current state of a cursor to the list. `O(n)`.
	///
	/// # Safety
	/// The cursor must be valid and point to a `Node` in the same list.
	#[inline]
	pub unsafe fn apply_cursor_unchecked(&mut self, cursor: &Cursor<T>) {
		self.current = cursor.current.unwrap_unchecked();
		self.index   = cursor.index;
	}

	#[inline]
	pub fn apply_cursor(&mut self, cursor: &Cursor<T>) -> bool {
		if self.index + cursor.index >= self.len { return false; }

		match self.index.cmp(&cursor.index) {
			Ordering::Greater => (0..self.index - cursor.index)
				.try_fold(self.current, |ptr, _| unsafe { ptr.as_ref() }.prev.map(|c| { self.current = c; ptr }))
				.map(|p| { self.current = p; self.index = cursor.index }).is_some(),
			Ordering::Less    => (0..cursor.index - self.index)
				.try_fold(self.current, |ptr, _| unsafe { ptr.as_ref() }.next.map(|c| { self.current = c; ptr }))
				.map(|p| { self.current = p; self.index = cursor.index }).is_some(),
			Ordering::Equal   => true,
		}
	}

	/// Split the list after the cursor. `O(1)`.  
	/// If the list is empty, or the cursor is at the end, `None` will be returned.  
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	/// list.advance();
	/// let new_list = list.split_after().unwrap();
	///
	/// assert_eq!(format!("{:?}", list), "[1, 2]");
	/// assert_eq!(format!("{:?}", new_list), "[3]");
	/// assert_eq!(new_list.index(), 0);
	/// ```
	pub fn split_after(&mut self) -> Option<Self> {
		if self.len == 0 { return None; }

		unsafe { self.current.as_ref() }.next.map(|next| {
			let mut new = Self::new();
			new.current = next;

			new.len = self.len - self.index - 1;
			self.len -= new.len;

			unsafe { self.current.as_mut().next = None; }
			unsafe { new .current.as_mut().prev = None; }

			new
		})
	}

	/// Split the list before the cursor. `O(1)`.
	/// If the list is empty, or the cursor is at the front, `None` will be returned.
	/// ```
	/// # use iterlist::IterList;
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
		if self.len == 0 { return None; }

		unsafe { self.current.as_ref() }.prev.map(|prev| {
			let mut new = Self::new();

			new.current = prev;
			new.len = self.index;
			self.len -= new.len;
			self.index = 0;
			new.index = new.len - 1;

			unsafe { self.current.as_mut().prev = None; }
			unsafe { new .current.as_mut().next = None; }

			new
		})
	}

	/// Get a ref to the current element. `O(1)`.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	///
	/// assert_eq!(list.current(), Some(&1));
	/// ```
	#[inline]
	pub fn current(&self) -> Option<&T> {
		self.is_empty().not().then_some(unsafe { &self.current.as_ref().elem })
	}

	/// Get a mut ref to the current element. `O(1)`.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	///
	/// *list.get_current_mut().unwrap() = 4;
	/// assert_eq!(format!("{:?}", list), "[4, 2, 3]");
	/// ```
	#[inline]
	pub fn get_current_mut(&mut self) -> Option<&mut T> {
		self.is_empty().not().then_some(unsafe { &mut self.current.as_mut().elem })
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

	/// Provides a copy of the current cursor. `O(1)`.  
	/// **Important**: Creating a cursor of an empty list does not work as expected,
	/// The cursor will be invalid until `Cursor::reaquire` is called on a non-empty list.
	/// ```
	/// # use iterlist::IterList;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// let mut slice = list.as_cursor();
	///
	/// assert_eq!(slice.next(),      Some(&1));
	/// assert_eq!(slice.next(),      Some(&2));
	/// assert_eq!(slice.next(),      Some(&3));
	///
	/// assert_eq!(list.current(), Some(&1));
	/// ```
	#[inline]
	pub fn as_cursor(&self) -> Cursor<T> {
		Cursor {
			_list:   PhantomData,
			index:   self.index,
			current: self.is_empty().not().then_some(self.current),
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
		self.get(index).expect("Index out of bounds")
	}
}

impl<T> Default for IterList<T> {
	#[inline]
	fn default() -> Self {
		Self::new()
	}
}

impl<T> std::ops::IndexMut<isize> for IterList<T> {
	#[inline]
	fn index_mut(&mut self, index: isize) -> &mut Self::Output {
		self.get_mut(index).expect("Index out of bounds")
	}
}

impl<T: Clone> Clone for IterList<T> {
	/// Clone the list. `O(n)`.  
	/// Cursor position is retained.
	/// ```
	/// # use iterlist::IterList;
	/// let list = vec![1, 2, 3].into_iter().collect::<IterList<_>>();
	/// assert_eq!(format!("{:?}", list), "[1, 2, 3]");
	///
	/// let cloned = list.clone();
	/// assert_eq!(format!("{:?}", cloned), "[1, 2, 3]");
	/// ```
	fn clone(&self) -> Self {
		let mut cursor = self.as_cursor();
		cursor.move_to_front();

		let mut list = cursor.cloned()
			.fold(Self::new(), |mut list, elem| { list.push_next(elem); list });

		let _ = list.move_to(self.index());
		list
	}
}

impl<T: Debug> Debug for IterList<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "[")?;

		let base = -(self.index() as isize);

		for i in 0..self.len as isize {
			let element = self.get(base + i);

			if let Some(element) = element {
				write!(f, "{:?}", element)?;
			}

			if i + 1 < (self.len as isize) {
				write!(f, ", ")?;
			}
		}

		write!(f, "]")
	}
}

impl<T> Drop for IterList<T> {
	#[inline]
	/// Drop the list. `O(n)`.
	fn drop(&mut self) {
		if self.is_empty() { return; }

		self.move_to_front();
		loop {
			let next = unsafe { self.current.as_ref().next };
			mem::drop(unsafe { Box::from_raw(self.current.as_ptr()) });
			match next {
				Some(next) => self.current = next,
				None => break,
			}
		}
	}
}


/*
* ==========================
* ===== Iteratory bits =====
* ==========================
*/

impl<T> Iterator for IterList<T> {
	type Item = T;

	/// Internally this call is just `consume_forward`. `O(1)`.
	/// ```
	/// # use iterlist::IterList;
	/// let list = IterList::from(vec![1, 2, 3]);
	///
	/// // list moved
	/// let num = list.fold(0, |acc, elem| acc + elem);
	/// assert_eq!(num, 6);
	/// ```
	#[inline]
	fn next(&mut self) -> Option<Self::Item> {
		self.consume_forward().map(|(elem, b)| {
			if !b { while self.consume_backward().is_some() {} }
			elem
		})
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) 
		{ (self.len, Some(self.len)) }
}

impl<T> DoubleEndedIterator for IterList<T> {
	/// Internally this call is just `consume_backward`. `O(1)`.
	/// ```
	/// # use iterlist::IterList;
	/// let mut list = IterList::from(vec![1, 2, 3]);
	///
	/// list.move_to_back();
	///
	/// let two = list.nth_back(1);
	/// assert_eq!(two, Some(2));
	/// ```
	#[inline]
	fn next_back(&mut self) -> Option<Self::Item> {
		self.consume_backward().map(|(elem, b)| {
			if !b { while self.consume_forward().is_some() {} }
			elem
		})
	}
}

impl<T> From<Vec<T>> for IterList<T> {
	/// Create a new list from a Vec. `O(n)`.  
	/// Cursor is set to the front of the list.
	/// ```
	/// # use iterlist::IterList;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// assert_eq!(format!("{:?}", list), "[1, 2, 3]");
	/// assert_eq!(list.current(), Some(&1));
	/// ```
	fn from(vec: Vec<T>) -> Self {
		let mut list = vec.into_iter().fold(Self::new(), 
			|mut list, elem| { list.push_next(elem); list });
		list.move_to_front();
		list
	}
}

impl<T: Clone> From<&[T]> for IterList<T> {
	/// Create a new list from a slice. `O(n)`.  
	/// Cursor is set to the front of the list.
	/// ```
	/// # use iterlist::IterList;
	/// let array: &[u8] = &[1, 2, 3];
	/// let list = IterList::from(array);
	/// assert_eq!(format!("{:?}", list), "[1, 2, 3]");
	/// assert_eq!(list.current(), Some(&1));
	/// ```
	fn from(slice: &[T]) -> Self {
		let mut list = slice.iter().cloned().fold(Self::new(), 
			|mut list, elem| { list.push_next(elem); list });
		list.move_to_front();
		list
	}
}

impl<T> FromIterator<T> for IterList<T> {
	/// Create a new list from an iterator. `O(n)`.  
	/// Cursor is set to the front of the list.
	/// ```
	/// # use iterlist::IterList;
	/// let list = (1..=3).into_iter().collect::<IterList<_>>();
	/// assert_eq!(format!("{:?}", list), "[1, 2, 3]");
	/// assert_eq!(list.current(), Some(&1));
	/// ```
	fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
		let mut list = iter.into_iter().fold(Self::new(), 
			|mut list, elem| { list.push_next(elem); list });
		list.move_to_front();
		list
	}
}



/*
* =======================
* ===== Cursor bits =====
* =======================
*/

/// A copy of a cursor of an IterList.  
/// Allows for traversing the list without modifying the original.  
///
/// Internally, the cursor is a fat pointer to the current element,
/// so the size of a `Cursor` is two words.  
/// ```
/// # use iterlist::IterList;
/// let list = IterList::from(vec![1, 2, 3]);
/// let mut cursor = list.as_cursor();
///
/// assert_eq!(cursor.next(), Some(&1));
/// assert_eq!(cursor.next(), Some(&2));
/// assert_eq!(cursor.current(), Some(&3));
///
/// assert_eq!(list.current(), Some(&1));
/// ```
#[derive(Clone, Copy)]
pub struct Cursor<'i, T> {
	current: Option<NonNull<Node<T>>>,
	index:   usize,
	_list:   PhantomData<&'i T>,
}

unsafe impl<T: Send> Send for Cursor<'_, T> {}
unsafe impl<T: Sync> Sync for Cursor<'_, T> {}

impl<'i, T> Iterator for Cursor<'i, T> {
	type Item = &'i T;

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
		{ (self.index, None) }
}

impl<'t, T> Cursor<'t, T> {
	/// Create a new cursor from a raw pointer. `O(1)`.
	///
	/// # Safety
	/// The pointer must be valid and point to a `Node` in a valid `IterList`.
	#[inline]
	pub const unsafe fn from_raw(ptr: *mut u8) -> Self {
		Self {
			current: Some(NonNull::new_unchecked(ptr as *mut Node<T>)),
			index:   0,
			_list:   PhantomData,
		}
	}

	/// Create a new dangling cursor. `O(1)`.
	/// The cursor will be invalid until `Cursor::reacquire` is called on a non-empty list.
	///
	/// # Safety
	/// Calling most methods on a dangling cursor will result in UB.
	#[inline]
	pub const unsafe fn new_dangling() -> Self {
		Self {
			current: Some(NonNull::dangling()),
			index:   0,
			_list:   PhantomData,
		}
	}

	/// Creates an empty cursor. `O(1)`.
	/// The cursor will be invalid until `Cursor::reacquire` is called on a non-empty list.
	#[inline]
	pub const fn new() -> Self {
		Self {
			current: None,
			index:   0,
			_list:   PhantomData,
		}
	}

	/// Create a new cursor from an IterList. `O(1)`.
	/// The new cursor is an exact copy of the list's cursor.
	/// 
	#[inline]
	pub fn from(list: &'t IterList<T>) -> Self {
		assert!(!list.is_empty(), "Cannot create a cursor from an empty list");
		Self {
			current: Some(list.current),
			index:   list.index,
			_list:   PhantomData,
		}
	}

	/// Update the cursor to match the current state of the list. `O(1)`.  
	/// Useful if you lose track of the list, or want to use the same cursor on multiple lists.
	#[inline]
	pub fn reacquire(&mut self, list: &'t IterList<T>) {
		self.current = (list.len != 0).then_some(list.current);
		self.index   = list.index;
	}

	/// Get a ref to the current element. `O(1)`.
	/// ```
	/// # use iterlist::IterList;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// let mut cursor = list.as_cursor();
	///
	/// assert_eq!(cursor.current(), Some(&1));
	/// ```
	#[inline]
	pub fn current(&self) -> Option<&T> {
		self.current.map(|c| unsafe { &c.as_ref().elem })
	}

	/// Get the index of the cursor `O(1)`.
	/// ```
	/// # use iterlist::IterList;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// let mut cursor = list.as_cursor();
	///
	/// cursor.next();
	/// assert_eq!(cursor.index(), 1);
	/// ```
	#[inline]
	pub const fn index(&self) -> usize {
		self.index
	}

	/// Move the cursor to the front of the list. `O(n)`.  
	/// Returns the number of elements traversed.
	/// ```
	/// # use iterlist::IterList;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// let mut cursor = list.as_cursor();
	///
	/// cursor.move_to_back();
	/// assert_eq!(cursor.current(), Some(&3));
	///
	/// let offset = cursor.move_to_front();
	/// assert_eq!(offset, 2);
	/// assert_eq!(cursor.current(), Some(&1));
	/// ```
	pub fn move_to_front(&mut self) -> usize {
		self.index = 0;

		for i in 0_usize.. {
			match self.current.and_then(|c| unsafe { c.as_ref().prev }) {
				Some(prev) => self.current = Some(prev),
				None => return i,
			}
		}
		unsafe { std::hint::unreachable_unchecked() }
	}

	/// Move the cursor to the back of the list. `O(n)`.  
	/// Returns the number of elements traversed.
	/// ```
	/// # use iterlist::IterList;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// let mut cursor = list.as_cursor();
	///
	/// let offset = cursor.move_to_back();
	/// assert_eq!(offset, 2);
	/// assert_eq!(cursor.current(), Some(&3));
	/// ```
	pub fn move_to_back(&mut self) -> usize {
		for i in 0_usize.. {
			match self.current.and_then(|c| unsafe { c.as_ref().next }) {
				Some(next) => self.current = Some(next),
				None => {
					self.index = i;
					return i;
				},
			}
		}
		unsafe { std::hint::unreachable_unchecked() }
	}

	/// Move the cursor to the specified index. `O(n)`.
	/// If the index is out of bounds the cursor will be moved to the edge, 
	/// and `false` will be returned.
	/// ```
	/// # use iterlist::IterList;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// let mut cursor = list.as_cursor();
	///
	/// cursor.move_to(1);
	/// assert_eq!(cursor.current(), Some(&2));
	/// ```
	/// # Panics
	/// Panics if the index is out of bounds.
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
	/// let list = IterList::from(vec![1, 2, 3]);
	/// let mut cursor = list.as_cursor();
	///
	/// assert_eq!(cursor.current(), Some(&1));
	///
	/// cursor.advance();
	/// assert_eq!(cursor.current(), Some(&2));
	/// ```
	#[inline]
	#[must_use]
	pub fn advance(&mut self) -> bool {
		self.current.and_then(|c| unsafe { c.as_ref().next }).map(|next| {
				self.current = Some(next);
				self.index += 1; })
			.is_some()
	}

	/// Move the cursor one step backward. `O(1)`.  
	/// Returns `false` if the cursor could not be moved.
	/// ```
	/// # use iterlist::IterList;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// let mut cursor = list.as_cursor();
	///
	/// cursor.move_to_back();
	/// assert_eq!(cursor.current(), Some(&3));
	///
	/// cursor.retreat();
	/// assert_eq!(cursor.current(), Some(&2));
	/// ```
	#[inline]
	#[must_use]
	pub fn retreat(&mut self) -> bool {
		self.current.and_then(|c| unsafe { c.as_ref().prev }).map(|prev| {
				self.current = Some(prev);
				self.index -= 1; })
			.is_some()
	}

	/// Move the cursor by a given offset. `O(n)`.  
	/// If the offset is out of bounds the cursor will be moved to the edge, 
	/// and `false` will be returned.
	/// ```
	/// # use iterlist::IterList;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// let mut cursor = list.as_cursor();
	///
	/// cursor.move_to_back();
	/// assert_eq!(cursor.index(), 2);
	///
	/// cursor.move_by(-2);
	/// assert_eq!(cursor.index(), 0);
	///
	/// cursor.move_by(10);
	/// assert_eq!(cursor.index(), 2);
	/// ```
	/// # Panics
	/// Panics if the offset is out of bounds.
	#[inline]
	pub fn move_by(&mut self, offset: isize) -> bool {
		match offset.cmp(&0) {
			Ordering::Greater => (0..offset ).fold(true, |_, _| self.advance()),
			Ordering::Less    => (0..-offset).fold(true, |_, _| self.retreat()),
			Ordering::Equal   => true,
		}
	}

	/// Get a ref to an element at the given offset. `O(n)`.
	/// Returns `None` if the offset is out of bounds.
	/// ```
	/// # use iterlist::IterList;
	/// let list = IterList::from(vec![1, 2, 3]);
	/// let mut cursor = list.as_cursor();
	///
	/// assert_eq!(cursor.current(), Some(&1));
	/// assert_eq!(cursor.get(1), Some(&2));
	/// assert_eq!(cursor.get(-1), None);
	/// ```
	pub fn get(&self, offset: isize) -> Option<&T> {
		match offset.cmp(&0) {
			Ordering::Greater => (0.. offset).try_fold(self.current, |mut ptr, _| ptr.and_then(|c| unsafe { c.as_ref().next }).map(|c| { ptr = Some(c); ptr }))?,
			Ordering::Less    => (0..-offset).try_fold(self.current, |mut ptr, _| ptr.and_then(|c| unsafe { c.as_ref().prev }).map(|c| { ptr = Some(c); ptr }))?,
			Ordering::Equal   => self.current
		}.map(|c| unsafe { &c.as_ref().elem })
	}
}

impl<T> std::ops::Deref for Cursor<'_, T> {
	type Target = T;

	#[inline]
	fn deref(&self) -> &Self::Target {
		self.current().unwrap()
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

impl<T: Debug> Debug for Cursor<'_, T> {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "{}: {:?}", self.index, self.current())
	}
}
