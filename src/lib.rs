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
//! assert_eq!(list.get_cursor(), Some(&2));
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
//! list.consume();
//! 
//! assert_eq!(format!("{:?}", list), "[-1, 1, 3]");
//! 
//! let num = list.fold(0, |acc, elem| acc + elem);
//! 
//! assert_eq!(num, 3);
//! ```

#![allow(forbidden_lint_groups)]
#![forbid(clippy::all)]
#![allow(clippy::option_map_unit_fn, clippy::wrong_self_convention)]

use std::fmt::Debug;
use std::marker::PhantomData;
use std::cmp::Ordering;
use std::ptr;

/// A doubly linked list. The `IterList` object is a fat pointer of a `Cursor + length`, which owns the underlying data.  
/// This means the total stack size is 3 words; each element is 2 words + element size.
pub struct IterList<T> {
    current: *mut Node<T>,
    index:   usize,
    len:     usize,
    _boo:    PhantomData<T>,
}

#[derive(Debug, Clone)]
struct Node<T> {
    next: *mut Node<T>,
    prev: *mut Node<T>,
    elem: T,
}

impl<T> IterList<T> {
    /// Create a new empty list. `O(1)`.  
    /// Does not allocate any memory.
    /// ```
    /// # use iterlist::IterList;
    /// let list: IterList<u8> = IterList::new();
    /// assert_eq!(list.len(), 0);
    /// ```
    #[inline]
    pub fn new() -> Self {
        Self { 
            current: ptr::null_mut(),
            len: 0,
            index: 0,
            _boo: PhantomData
        }
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
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// assert_eq!(&format!("{:?}", list), "[1, 3, 2]");
    /// ```
    pub fn insert_next(&mut self, elem: T) {
        unsafe {
            let new = Box::into_raw(Box::new(Node {
                prev: ptr::null_mut(),
                next: ptr::null_mut(),
                elem,
            }));

            match self.current.as_ptr() {
                Some(current) => {
                    if let Some(next) = (*current).next.as_ptr() {
                        (*next).prev    = new;
                        (*new).next  = next;
                    }

                    (*current).next = new;
                    (*new).prev  = current;
                },
                None => self.current = new,
            }
            
            self.len += 1;
        }
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
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// assert_eq!(&format!("{:?}", list), "[2, 3, 1]");
    /// ```
    pub fn insert_prev(&mut self, elem: T) {
        unsafe {
            let new = Box::into_raw(Box::new(Node {
                prev: ptr::null_mut(),
                next: ptr::null_mut(),
                elem,
            }));

            match self.current.as_ptr() {
                Some(current) => {
                    if let Some(prev) = (*current).prev.as_ptr() {
                        (*prev).next   = new;
                        (*new).prev = prev;
                    }

                    (*current).prev = new;
                    (*new).next  = current;
                    self.index += 1;
                },
                None => self.current = new,
            }

            self.len += 1;
        }
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
    /// assert_eq!(list.get_cursor(), Some(&3));
    /// assert_eq!(&format!("{:?}", list), "[1, 2, 3]");
    /// ```
    pub fn push_next(&mut self, elem: T) {
        if self.current.is_null() {
            self.insert_next(elem);
            return;
        }

        self.insert_next(elem);
        self.advance();

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
    /// assert_eq!(list.get_cursor(), Some(&3));
    /// assert_eq!(&format!("{:?}", list), "[3, 2, 1]");
    /// ```
    pub fn push_prev(&mut self, elem: T) {
        if self.current.is_null() {
            self.insert_prev(elem);
            return;
        }

        self.insert_prev(elem);
        self.retreat();
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
    /// assert_eq!(list.get_cursor(), Some(&3));
    ///
    /// let offset = list.move_to_front();
    /// assert_eq!(offset, 2);
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// ```
    pub fn move_to_front(&mut self) -> usize {
        unsafe {
            self.index = 0;
            
            let mut offset = 0;
            while let Some(prev) = self.current.as_ref().map(|c| c.prev).filter(|p| !p.is_null()) {
                self.current = prev;
                offset += 1;
            }
            offset
        }
    }

    /// Move the cursor to the back of the list. `O(n)`.  
    /// Returns the number of elements traversed.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// let offset = list.move_to_back();
    /// assert_eq!(offset, 2);
    /// assert_eq!(list.get_cursor(), Some(&3));
    /// ```
    pub fn move_to_back(&mut self) -> usize {
        unsafe {
            self.index = self.len - 1;

            let mut offset = 0;
            while let Some(next) = self.current.as_ref().map(|c| c.next).filter(|p| !p.is_null()) {
                self.current = next;
                offset += 1;
            }
            offset
        }
    }

    /// Move the cursor to the specified index. `O(n)`.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    /// list.move_to(1);
    /// assert_eq!(list.get_cursor(), Some(&2));
    /// ```
    /// # Panics
    /// Panics if the index is out of bounds.
    pub fn move_to(&mut self, index: usize) {
        if index > self.len {
            panic!("Index out of bounds");
        }

        match self.index.cmp(&index) {
            Ordering::Greater => (0..self.index - index).for_each(|_| { self.retreat(); }),
            Ordering::Less    => (0..index - self.index).for_each(|_| { self.advance(); }),
            Ordering::Equal   => (),
        }
    }

    /// Move the cursor one step forward. `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(list.get_cursor(), Some(&1));
    ///
    /// list.advance();
    /// assert_eq!(list.get_cursor(), Some(&2));
    /// ```
    #[inline]
    pub fn advance(&mut self) -> bool {
        unsafe {
            self.current.as_ref().and_then(|c| c.next.as_ptr()).map(|c| {
                self.current = c;
                self.index += 1;
            }).is_some()
        }
    }

    /// Move the cursor one step backward. `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// list.move_to_back();
    /// assert_eq!(list.get_cursor(), Some(&3));
    ///
    /// list.retreat();
    /// assert_eq!(list.get_cursor(), Some(&2));
    /// ```
    #[inline]
    pub fn retreat(&mut self) -> bool {
        unsafe {
            self.current.as_ref().and_then(|c| c.prev.as_ptr()).map(|prev| {
                self.current = prev;
                self.index -= 1;
            }).is_some()
        }
    }

    /// Move the cursor by a given offset. `O(n)`.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// list.move_to_back();
    /// assert_eq!(list.index(), 2);
    ///
    /// list.move_by(-2);
    /// assert_eq!(list.index(), 0);
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// ```
    /// # Panics
    /// Panics if the offset is out of bounds.
    pub fn move_by(&mut self, offset: isize) {
        if offset.checked_abs().and_then(|s| (s > self.index as isize).into())
            .and_then(|_| (self.index as isize + offset > self.len as isize).into()).is_none() {
                panic!("Index out of bounds");
            }

        match offset.cmp(&0) {
            Ordering::Greater => (0..offset).for_each(|_| { self.advance(); }),
            Ordering::Less    => (0..-offset).for_each(|_| { self.retreat(); }),
            Ordering::Equal   => (),
        }
    }

    /// Get a ref to an element at the given offset. `O(n)`.
    /// Returns `None` if the offset is out of bounds.
    /// ```
    /// # use iterlist::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// assert_eq!(list.get(1), Some(&2));
    /// assert_eq!(list.get(-1), None);
    /// ```
    pub fn get(&self, offset: isize) -> Option<&T> {
        offset.checked_abs()
            .and_then(|s| (s > self.index as isize).into())
            .and_then(|_| (self.index as isize + offset > self.len as isize).into())?;

        let mut ptr = self.current.as_ptr()?;

        unsafe {
            match offset.cmp(&0) {
                Ordering::Greater => (0..offset).for_each(|_| {ptr.as_ref().map(|c| ptr = c.next); }),
                Ordering::Less    => (0..-offset).for_each(|_| {ptr.as_ref().map(|c| ptr =  c.prev); }),
                Ordering::Equal   => return self.get_cursor(),
            }

            ptr.as_ref().map(|c| &c.elem )
        }
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
    pub fn get_mut(&mut self, offset: isize) -> Option<&mut T> {
        offset.checked_abs()
            .and_then(|s| (s > self.index as isize).into())
            .and_then(|_| (self.index as isize + offset > self.len as isize).into())?;

        let mut current = self.current.as_ptr()?;

        unsafe {
            match offset.cmp(&0) {
                Ordering::Greater => (0..offset).for_each(|_| {current.as_ref().map(|c| current = c.next); }),
                Ordering::Less    => (0..-offset).for_each(|_| {current.as_ref().map(|c| current=  c.prev); }),
                Ordering::Equal   => return self.get_cursor_mut(),
            }

            current.as_mut().map(|c| &mut c.elem )
        }
    }

    /// Remove the current element and return it. `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(list.consume(), Some(1));
    /// assert_eq!(&format!("{:?}", list), "[2, 3]");
    /// ```
    pub fn consume(&mut self) -> Option<T> {
        self.current.as_ptr().map(|node| unsafe {
            let node = Box::from_raw(node);

            if let Some(prev) = node.prev.as_mut() {
                prev.next = node.next
            }

            if let Some(next) = node.next.as_mut() {
                next.prev = node.prev
            }

            let elem = node.elem;
            self.current = node.next;
            self.len -= 1;
            elem
        })
    }

    /// Get a ref to the current element. `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// ```
    #[inline]
    pub fn get_cursor(&self) -> Option<&T> {
        unsafe { self.current.as_ref().map(|c| &c.elem ) }
    }

    /// Get a mut ref to the current element. `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// *list.get_cursor_mut().unwrap() = 4;
    /// assert_eq!(format!("{:?}", list), "[4, 2, 3]");
    /// ```
    #[inline]
    pub fn get_cursor_mut(&mut self) -> Option<&mut T> {
        unsafe { self.current.as_mut().map(|c| &mut c.elem ) }
    }

    /// Get the number of elements in the list. `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(list.len(), 3);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if the list is empty. `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let list: IterList<u8> = IterList::new();
    /// assert!(list.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.current.is_null()
    }

    /// Get the index of the cursor `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(list.index(), 0);
    /// ```
    #[inline]
    pub fn index(&self) -> usize {
        self.index
    }

    /// Provides a copy of the current cursor. `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// let mut slice = list.as_cursor();
    ///
    /// assert_eq!(slice.next(),      Some(&1));
    /// assert_eq!(slice.next(),      Some(&2));
    /// assert_eq!(slice.next(),      Some(&3));
    ///
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// ```
    #[inline]
    pub fn as_cursor<'i>(&'i self) -> Cursor<'i, T> {
        Cursor {
            _list: PhantomData,
            index: self.index,
            current: self.current,
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

impl<T> Default for IterList<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
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
    /// # use iterlist::IterList;
    /// let list = vec![1, 2, 3].into_iter().collect::<IterList<_>>();
    /// assert_eq!(format!("{:?}", list), "[1, 2, 3]");
    ///
    /// let cloned = list.clone();
    /// assert_eq!(format!("{:?}", cloned), "[1, 2, 3]");
    /// ```
    fn clone(&self) -> Self {
        let mut list = self.as_cursor().cloned().fold(Self::new(), |mut list, elem| {
            list.push_next(elem);
            list
        });

        list.move_to(self.index());
        list
    }
}

impl<T: Debug> Debug for IterList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[")?;

        let base = -(self.index() as isize);

        let mut i: isize = 0;
        while i < (self.len as isize) {
            let element = self.get(base + i);

            if let Some(element) = element {
                write!(f, "{:?}", element)?;
            }

            i += 1;

            if i < (self.len as isize) {
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
        while self.consume().is_some() {}
    }
}


/*
 * ==========================
 * ===== Iteratory bits =====
 * ==========================
 */

impl<T> Iterator for IterList<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.consume()
    }
}

impl<T> From<Vec<T>> for IterList<T> {
    /// Create a new list from a Vec. `O(n)`.  
    /// Cursor is set to the front of the list.
    /// ```
    /// # use iterlist::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(format!("{:?}", list), "[1, 2, 3]");
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// ```
    fn from(vec: Vec<T>) -> Self {
        let mut list = vec.into_iter().fold(Self::new(), |mut list, elem| {
            list.push_next(elem);
            list
        });
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
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// ```
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut list = iter.into_iter().fold(Self::new(), |mut list, elem| {
            list.push_next(elem);
            list
        });
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
/// Internally, the cursor is a pointer to the current element,
/// so the size of a `Cursor` is two words.  
/// ```
/// # use iterlist::IterList;
/// let list = IterList::from(vec![1, 2, 3]);
/// let mut cursor = list.as_cursor();
///
/// assert_eq!(cursor.next(), Some(&1));
/// assert_eq!(cursor.next(), Some(&2));
/// assert_eq!(cursor.get_cursor(), Some(&3));
///
/// assert_eq!(list.get_cursor(), Some(&1));
/// ```
#[derive(Clone, Copy)]
pub struct Cursor<'i, T> {
    current: *mut Node<T>,
    index:   usize,
    _list:   PhantomData<&'i T>,
}

impl<'i, T> Iterator for Cursor<'i, T> {
    type Item = &'i T;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            self.current.as_ref().map(|c| {
                self.current = c.next;
                self.index += 1;
                &c.elem
            })
        }
    }
}

impl<'t, T> Cursor<'t, T> {
    /// Get a ref to the current element. `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// let mut cursor = list.as_cursor();
    ///
    /// assert_eq!(cursor.get_cursor(), Some(&1));
    /// ```
    #[inline]
    pub fn get_cursor(&self) -> Option<&T> {
        unsafe { self.current.as_ref().map(|c| &c.elem) }
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
    pub fn index(&self) -> usize {
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
    /// assert_eq!(cursor.get_cursor(), Some(&3));
    ///
    /// let offset = cursor.move_to_front();
    /// assert_eq!(offset, 2);
    /// assert_eq!(cursor.get_cursor(), Some(&1));
    /// ```
    pub fn move_to_front(&mut self) -> usize {
        unsafe {
            self.index = 0;
            
            let mut offset = 0;
            while let Some(prev) = self.current.as_ref().map(|c| c.prev).filter(|p| !p.is_null()) {
                self.current = prev;
                offset += 1;
            }
            offset
        }
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
    /// assert_eq!(cursor.get_cursor(), Some(&3));
    /// ```
    pub fn move_to_back(&mut self) -> usize {
        unsafe {
            let mut offset = 0;
            while let Some(next) = self.current.as_ref().map(|c| c.next).filter(|p| !p.is_null()) {
                self.current = next;
                offset += 1;
            }
            self.index += offset;
            offset
        }
    }

    /// Move the cursor to the specified index. `O(n)`.
    /// ```
    /// # use iterlist::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// let mut cursor = list.as_cursor();
    ///
    /// cursor.move_to(1);
    /// assert_eq!(cursor.get_cursor(), Some(&2));
    /// ```
    /// # Panics
    /// Panics if the index is out of bounds.
    pub fn move_to(&mut self, index: usize) {
        match self.index.cmp(&index) {
            Ordering::Greater => (0..self.index - index).for_each(|_| { self.retreat(); }),
            Ordering::Less    => (0..index - self.index).for_each(|_| { self.advance(); }),
            Ordering::Equal   => (),
        }
    }

    /// Move the cursor one step forward. `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// let mut cursor = list.as_cursor();
    ///
    /// assert_eq!(cursor.get_cursor(), Some(&1));
    ///
    /// cursor.advance();
    /// assert_eq!(cursor.get_cursor(), Some(&2));
    /// ```
    #[inline]
    pub fn advance(&mut self) -> bool {
        unsafe {
            self.current.as_ref().and_then(|c| c.next.as_ptr()).map(|c| {
                self.current = c;
                self.index += 1;
            }).is_some()
        }
    }

    /// Move the cursor one step backward. `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// let mut cursor = list.as_cursor();
    ///
    /// cursor.move_to_back();
    /// assert_eq!(cursor.get_cursor(), Some(&3));
    ///
    /// cursor.retreat();
    /// assert_eq!(cursor.get_cursor(), Some(&2));
    /// ```
    #[inline]
    pub fn retreat(&mut self) -> bool {
        unsafe {
            self.current.as_ref().and_then(|c| c.prev.as_ptr()).map(|c| {
                self.current = c;
                self.index -= 1;
            }).is_some()
        }
    }

    /// Move the cursor by a given offset. `O(n)`.
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
    /// ```
    /// # Panics
    /// Panics if the offset is out of bounds.
    pub fn move_by(&mut self, offset: isize) {
        if offset.checked_abs().and_then(|s| (s > self.index as isize).into()).is_none() {
            panic!("Index out of bounds");
        };

        self.index = (self.index as isize).saturating_add(offset) as usize;

        match offset.cmp(&0) {
            Ordering::Greater => (0..offset).for_each(|_| { self.advance(); }),
            Ordering::Less    => (0..offset).for_each(|_| { self.retreat(); }),
            Ordering::Equal   => (),
        }
    }

    /// Get a ref to an element at the given offset. `O(n)`.
    /// Returns `None` if the offset is out of bounds.
    /// ```
    /// # use iterlist::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// let mut cursor = list.as_cursor();
    ///
    /// assert_eq!(cursor.get_cursor(), Some(&1));
    /// assert_eq!(cursor.get(1), Some(&2));
    /// assert_eq!(cursor.get(-1), None);
    /// ```
    pub fn get(&self, offset: isize) -> Option<&T> {
        offset.checked_abs().and_then(|s| (s > self.index as isize).into())?;

        let mut ptr = self.current.as_ptr()?;

        unsafe {
            match offset.cmp(&0) {
                Ordering::Greater => (0..offset).for_each(|_| {ptr.as_ref().map(|c| ptr = c.next); }),
                Ordering::Less    => (0..-offset).for_each(|_| {ptr.as_ref().map(|c| ptr =  c.prev); }),
                Ordering::Equal   => return self.get_cursor(),
            }

            ptr.as_ref().map(|c| &c.elem )
        }
    }
}


impl<'i, T> std::ops::Index<isize> for Cursor<'i, T> {
    type Output = T;

    /// Essentially equivalent to `get`. `O(n)`.  
    /// # Panics
    /// Panics if the index is out of bounds.
    #[inline]
    fn index(&self, index: isize) -> &Self::Output {
        self.get(index).unwrap_or_else(|| panic!("Index out of bounds"))
    }
}




/*
 * =================
 * ===== OTHER =====
 * =================
 */

trait PtrExt<T> {
    fn as_ptr(self) -> Option<*mut T>;
}

impl<T> PtrExt<T> for *mut T {
    #[inline]
    fn as_ptr(self) -> Option<*mut T> {
        (!self.is_null()).then_some(self) 
    }
}
