use std::fmt::Debug;
use std::marker::PhantomData;
use std::cmp::Ordering;
use std::mem::{MaybeUninit, self, ManuallyDrop};
use std::ptr;

/// A doubly linked list. The `IterList` object is a fat pointer of a `Cursor + length`, which owns the underlying data.  
/// This means the total stack size is 3 words; each element is 2 words + element size.
pub struct IterList<T> {
    current: *mut Node<T>,
    index:   usize,
    len:     usize,
    _boo:    PhantomData<T>,
}

struct Node<T> {
    next: *mut Node<T>,
    prev: *mut Node<T>,
    elem: T,
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
                        (*next).prev = new;
                        (*new).next  = next;
                    }

                    (*current).next = new;
                    (*new).prev     = current;
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
                        (*prev).next = new;
                        (*new).prev  = prev;
                    }

                    (*current).prev = new;
                    (*new).next     = current;
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
    /// If the index is out of bounds the cursor will be moved to the edge, 
    /// and `false` will be returned.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    /// list.move_to(1);
    /// assert_eq!(list.get_cursor(), Some(&2));
    /// ```
    #[inline]
    pub fn move_to(&mut self, index: usize) -> bool {
        match self.index.cmp(&index) {
            Ordering::Greater => (0..self.index - index).fold(true, |_, _| self.retreat()),
            Ordering::Less    => (0..index - self.index).fold(true, |_, _| self.advance()),
            Ordering::Equal   => true,
        }
    }

    /// Move the cursor one step forward. `O(1)`.  
    /// Returns `false` if the cursor could not be moved.
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
    /// Returns `false` if the cursor could not be moved.
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
    /// assert_eq!(list.get_cursor(), Some(&1));
    ///
    /// assert!(!list.move_by(10));
    /// assert_eq!(list.index(), 2);
    /// ```
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
    /// The cursor will then point to the next element.  
    /// If the removed element was at the end of the list, the cursor will point to the previous
    /// element and `false` will be returned.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(list.consume_forward(), Some((1, true)));
    /// assert_eq!(&format!("{:?}", list), "[2, 3]");
    /// assert_eq!(list.get_cursor(), Some(&2));
    /// ```
    pub fn consume_forward(&mut self) -> Option<(T, bool)> {
        self.current.as_ptr().map(|node| unsafe {
            let node = Box::from_raw(node);
            self.len -= 1;

            node.prev.as_mut().map(|prev| prev.next = node.next);

            match node.next.as_mut() {
                Some(next) => {
                    next.prev = node.prev;
                    self.current = node.next;
                    (node.elem, true)
                },
                None => {
                    self.current = node.prev;
                    // self.index -= 1;
                    (node.elem, false)
                }
            }
        })
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
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// ```
    pub fn consume_backward(&mut self) -> Option<(T, bool)> {
        self.current.as_ptr().map(|node| unsafe {
            let node = Box::from_raw(node);
            self.len -= 1;
            self.index -= 1;

            node.next.as_mut().map(|next| next.prev = node.prev);

            match node.prev.as_mut() {
                Some(prev) => {
                    prev.next = node.next;
                    self.current = node.prev;
                    (node.elem, true)
                },
                None => {
                    self.current = node.next;
                    (node.elem, false)
                }
            }
        })
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
        unsafe {
            match self.current.as_mut() {
                None => { self.push_next(elem); None }, 
                Some(node) => Some(std::mem::replace(&mut node.elem, elem)),
            }
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
        unsafe {
            self.current.as_ref().and_then(|c| c.next.as_ptr()).map(|next| {
                let mut new = Self::new();

                new.current = next;
                new.len = self.len - self.index - 1;
                self.len -= new.len;

                (*self.current).next = ptr::null_mut();
                (*new.current).prev = ptr::null_mut();

                new
            })
        }
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
        unsafe {
            self.current.as_ref().and_then(|c| c.prev.as_ptr()).map(|prev| {
                let mut new = Self::new();

                new.current = prev;
                new.len = self.index;
                self.len -= new.len;
                self.index = 0;
                new.index = new.len - 1;

                (*self.current).prev = ptr::null_mut();
                (*new.current).next = ptr::null_mut();

                new
            })
        }
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
    pub fn as_cursor(&self) -> Cursor<T> {
        Cursor {
            _list: PhantomData,
            index: self.index,
            current: self.current,
        }
    }

    /// Make the list continous in memory. `O(n)`.  
    /// This allows you to take a high upfront cost to potentially make future reads faster.
    ///
    /// The returned list will not be able to have elements added or removed, as doing so could cause a use after free or memory leak.  
    /// (the memory has to be deallocated all at once)  
    /// 
    /// In the future this will be moved and expanded into a `pool` module.
    /// ```
    /// # use iterlist::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// let mut contigous = list.into_continous();
    ///
    /// assert_eq!(format!("{:?}", contigous), "[1, 2, 3]");
    /// assert_eq!(contigous.get_mut(1), Some(&mut 2));
    ///
    /// let num = contigous.as_cursor().fold(0, |acc, elem| acc + elem);
    /// assert_eq!(num, 6);
    /// ```
    pub fn into_continous(mut self) -> ContinousIterList<T> {
        unsafe {
            let mut vec = Vec::<Node<T>>::with_capacity(self.len);
            let vec_ptr = vec.as_mut_ptr();

            let (len, index) = (self.len, self.index);

            self.move_to_front();

            let mut prev = ptr::null_mut();
            let mut i = 0;

            while let Some((elem, _)) = self.consume_forward() {
                vec.push(Node { next: ptr::null_mut(), prev, elem, });

                prev.as_ptr().map(|p| (*p).next = vec_ptr.add(i));
                prev = vec_ptr.add(i);

                i += 1;
            }

            mem::forget(vec);

            ContinousIterList(ManuallyDrop::new(IterList {
                current: vec_ptr.add(index),
                len, index,
                _boo: PhantomData,
            }))
        }
    }
}

/// An `IterList` packed into continous memory.  
/// Cannot have elements added or removed.  
pub struct ContinousIterList<T>(ManuallyDrop<IterList<T>>);

impl<T> std::ops::Deref for ContinousIterList<T> {
    type Target = IterList<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Drop for ContinousIterList<T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            std::alloc::dealloc(self.0.current as *mut u8, std::alloc::Layout::array::<Node<T>>(self.0.len).unwrap());
        }
    }
}

impl<T> ContinousIterList<T> {
    #[inline]
    pub fn get_mut(&mut self, offset: isize) -> Option<&mut T> {
        self.0.get_mut(offset)
    }

    #[inline]
    pub fn cursor_mut(&mut self) -> Option<&mut T> {
        self.0.get_cursor_mut()
    }

    /// Convert a `ContinousIterList` back into an `IterList`. `O(n)`.
    pub fn into_iterlist(self) -> IterList<T> {
        let mut new = IterList::new();

        if self.0.len() == 0 {
            return new;
        }

        unsafe {
            let mut current = self.0.current;
            while let Some(node) = current.as_ptr() {
                let replace: T = MaybeUninit::uninit().assume_init();
                new.push_next(mem::replace(&mut (*node).elem, replace));
                current = (*node).next;
            }
        }

        new.move_to(self.0.index);

        new
    }
}

impl<T: Debug> Debug for ContinousIterList<T> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", &*self.0)
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
        let mut cursor = self.as_cursor();
        cursor.move_to_front();

        let mut list = cursor.cloned().fold(Self::new(), |mut list, elem| {
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
        unsafe {
            self.move_to_front();
            while let Some(current) = self.current.as_ptr() {
                self.current = (*current).next;
                mem::drop(Box::from_raw(current));
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

impl<T: Clone> From<&[T]> for IterList<T> {
    /// Create a new list from a slice. `O(n)`.  
    /// Cursor is set to the front of the list.
    /// ```
    /// # use iterlist::IterList;
    /// let array: &[u8] = &[1, 2, 3];
    /// let list = IterList::from(array);
    /// assert_eq!(format!("{:?}", list), "[1, 2, 3]");
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// ```
    fn from(slice: &[T]) -> Self {
        let mut list = slice.iter().cloned().fold(Self::new(), |mut list, elem| {
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
/// Internally, the cursor is a fat pointer to the current element,
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

unsafe impl<'i, T: Send> Send for Cursor<'i, T> {}
unsafe impl<'i, T: Sync> Sync for Cursor<'i, T> {}

impl<'i, T> Iterator for Cursor<'i, T> {
    type Item = &'i T;

    #[inline]
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

    /// Update the cursor to match the current state of the list. `O(1)`.  
    /// Useful if you lose track of the list, or want to use the same cursor on multiple lists.
    #[inline]
    pub fn reaquire(&mut self, list: &'t IterList<T>) {
        self.current = list.current;
        self.index   = list.index;
    }

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
    /// If the index is out of bounds the cursor will be moved to the edge, 
    /// and `false` will be returned.
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
    #[inline]
    pub fn move_to(&mut self, index: usize) -> bool {
        match self.index.cmp(&index) {
            Ordering::Greater => (0..self.index - index).fold(true, |_, _| self.retreat()),
            Ordering::Less    => (0..index - self.index).fold(true, |_, _| self.advance()),
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
    /// Returns `false` if the cursor could not be moved.
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

impl<'i, T> std::ops::Deref for Cursor<'i, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.get_cursor().unwrap()
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

impl<T: Debug> Debug for Cursor<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}: {:?}", self.index, self.get_cursor())
    }
}






pub trait PtrExt<T> {
    fn as_ptr(self) -> Option<*mut T>;
}

impl<T> PtrExt<T> for *mut T {
    #[inline]
    fn as_ptr(self) -> Option<*mut T> {
        (!self.is_null()).then_some(self) 
    }
}
