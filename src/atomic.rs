use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering::*};
use std::marker::PhantomData;
use std::ptr;
use std::cmp::Ordering;
use std::mem;
use std::fmt::Debug;

/// an Atomic version of IterList.
pub struct IterList<T> {
    current: AtomicPtr<Node<T>>,
    index:   AtomicUsize,
    len:     AtomicUsize,
    _owned:  PhantomData<T>,
}

struct Node<T> {
    next: AtomicPtr<Node<T>>,
    prev: AtomicPtr<Node<T>>,
    elem: T, 
}

impl<T> Default for IterList<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<T: Sync> Sync for IterList<T> {}
unsafe impl<T: Send> Send for IterList<T> {}

impl<T> IterList<T> {
    /// Create a new empty list. `O(1)`.  
    /// Does not allocate any memory.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let list: IterList<u8> = IterList::new();
    /// assert_eq!(list.len(), 0);
    /// ```
    #[inline]
    pub fn new() -> Self {
        Self {
            current: AtomicPtr::default(),
            index:   AtomicUsize::new(0),
            len:     AtomicUsize::new(0),
            _owned:  PhantomData,
        }
    }

    /// Insert an element after the cursor, retaining current position. `O(1)`.  
    /// If the list is empty it will be inserted at index 0.
    ///
    /// Returns `false` if the element could not be inserted.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::new();
    /// let _ = list.insert_next(1);
    /// let _ = list.insert_next(2);
    /// let _ = list.insert_next(3);
    /// 
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// assert_eq!(&format!("{:?}", list), "[1, 3, 2]");
    /// ```
    #[must_use]
    pub fn insert_next(&self, elem: T) -> Result<(), T> {
        let new = Box::into_raw(Box::new(Node {
            prev: AtomicPtr::default(),
            next: AtomicPtr::default(),
            elem,
        }));

        unsafe {
            match self.current.load_ptr(Acquire) {
                Some(current) => {
                    if let Some(next) = (*current).next.load_ptr(Acquire) {
                        (*next).prev.store(new, Release);
                        (*new).next.store(next, Relaxed);
                    }

                    (*current).next.store(new, Release);
                    (*new).prev.store(current, Release);
                },
                None => if self.current.compare_exchange(ptr::null_mut(), new, AcqRel, Acquire).is_err() {
                    return Err(Box::from_raw(new).elem);
                },
            }
        }

        self.len.fetch_add(1, AcqRel);
        Ok(())
    }

    /// Insert an element before the cursor, retaining current position. `O(1)`.
    /// If the list is empty it will be inserted at index 0.
    ///
    /// Returns `Err(T)` if the element could not be inserted.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::new();
    /// let _ = list.insert_prev(1);
    /// let _ = list.insert_prev(2);
    /// let _ = list.insert_prev(3);
    ///
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// assert_eq!(&format!("{:?}", list), "[2, 3, 1]");
    /// ```
    #[must_use]
    pub fn insert_prev(&self, elem: T) -> Result<(), T> {
        let new = Box::into_raw(Box::new(Node {
            prev: AtomicPtr::default(),
            next: AtomicPtr::default(),
            elem,
        }));

        unsafe {
            match self.current.load_ptr(Acquire) {
                Some(current) => {
                    if let Some(prev) = (*current).prev.load_ptr(Acquire) {
                        (*prev).next.store(new, Release);
                        (*new).prev.store(prev, Relaxed);
                    }

                    (*current).prev.store(new, Release);
                    (*new).next.store(current, Release);

                    self.index.fetch_add(1, Release);
                },
                None => if self.current.compare_exchange(ptr::null_mut(), new, AcqRel, Acquire).is_err() {
                    return Err(Box::from_raw(new).elem);
                },
            }
        }

        self.len.fetch_add(1, AcqRel);
        Ok(())
    }

    /// Push an element after the cursor, moving the cursor to it. `O(1)`.  
    /// If the list is empty it will be inserted at index 0.
    ///
    /// Returns `false` if could not advance the cursor.
    /// Returns `T` if the element could not be pushed.
    ///
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::new();
    /// let _ = list.push_next(1);
    /// let _ = list.push_next(2);
    /// let _ = list.push_next(3);
    ///
    /// assert_eq!(list.get_cursor(), Some(&3));
    /// assert_eq!(&format!("{:?}", list), "[1, 2, 3]");
    /// ```
    #[must_use]
    pub fn push_next(&self, elem: T) -> Result<bool, T> {
        if self.current.load(Relaxed).is_null() {
            return self.insert_next(elem).map(|_| true);
        }

        self.insert_next(elem)?;
        self.advance().map_or_else(|_| Ok(false), |_| Ok(true))
    }

    /// Push an element before the cursor, moving the cursor to it. `O(1)`.
    /// If the list is empty it will be inserted at index 0.
    ///
    /// Returns `false` if could not advance the cursor.
    /// Returns `T` if the element could not be pushed.
    ///
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::new();
    /// let _ = list.push_prev(1);
    /// let _ = list.push_prev(2);
    /// let _ = list.push_prev(3);
    ///
    /// assert_eq!(list.get_cursor(), Some(&3));
    /// assert_eq!(&format!("{:?}", list), "[3, 2, 1]");
    /// ```
    #[must_use]
    pub fn push_prev(&self, elem: T) -> Result<bool, T> {
        if self.current.load(Relaxed).is_null() {
            return self.insert_prev(elem).map(|_| true);
        }

        self.insert_prev(elem)?;
        self.retreat().map_or_else(|_| Ok(false), |_| Ok(true))
    }

    /// Move the cursor to the front of the list. `O(n)`.  
    /// Returns the number of elements traversed.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::new();
    /// let _ = list.push_next(1);
    /// let _ = list.push_next(2);
    /// let _ = list.push_next(3);
    ///
    /// assert_eq!(list.get_cursor(), Some(&3));
    ///
    /// let offset = list.move_to_front();
    /// assert_eq!(offset, 2);
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// ```
    pub fn move_to_front(&self) -> usize {
        unsafe {
            self.index.store(0, Relaxed);
            
            let mut offset = 0;
            while let Some(prev) = self.current.load(Acquire).as_ref()
                .map(|c| c.prev.load(Acquire)).filter(|p| !p.is_null()) 
            {
                self.current.store(prev, Release);
                offset += 1;
            }
            offset
        }
    }

    /// Move the cursor to the back of the list. `O(n)`.
    /// Returns the number of elements traversed.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// let offset = list.move_to_back();
    /// assert_eq!(offset, 2);
    /// assert_eq!(list.get_cursor(), Some(&3));
    /// ```
    pub fn move_to_back(&self) -> usize {
        unsafe {
            self.index.store(self.len.load(Relaxed) - 1, Relaxed);

            let mut offset = 0;
            while let Some(next) = self.current.load(Acquire).as_ref()
                .map(|c| c.next.load(Acquire)).filter(|p| !p.is_null()) 
            {
                self.current.store(next, Release);
                offset += 1;
            }
            offset
        }
    }

    /// Move the cursor to the specified index. `O(n)`.
    /// If the index is out of bounds the cursor will be moved to the edge, and `false` will be returned.
    /// Returns `Err(())` if the cursor could not be moved.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    /// list.move_to(1);
    /// assert_eq!(list.get_cursor(), Some(&2));
    /// ```
    #[must_use]
    pub fn move_to(&self, index: usize) -> Result<bool, ()> {
        match self.index.load(Relaxed).cmp(&index) {
            Ordering::Greater => {
                for _ in 0..index - self.index.load(Relaxed) {
                    match self.retreat() {
                        Ok(false) => return Ok(false),
                        Err(_)    => return Err(()),
                        _         => continue,
                    }
                }
            },
            Ordering::Less    => {
                for _ in 0..index - self.index.load(Relaxed) {
                    match self.advance() {
                        Ok(false) => return Ok(false),
                        Err(_)    => return Err(()),
                        _         => continue,
                    }
                }
            },
            _ => (),
        }
        Ok(true)
    }


    /// Move the cursor one step forward. `O(1)`.  
    /// Returns `false` if the cursor is at the edge.
    /// Returns `Err(())` if the cursor could not be moved.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(list.get_cursor(), Some(&1));
    ///
    /// list.advance();
    /// assert_eq!(list.get_cursor(), Some(&2));
    /// ```
    #[must_use]
    pub fn advance(&self) -> Result<bool, ()> {
        unsafe {
            let current = self.current.load(Acquire);
            match current.as_ref().and_then(|c| c.next.load_ptr(Acquire)).map(|c| {
                self.index.fetch_add(1, Release);
                self.current.compare_exchange(current, c, Acquire, Relaxed).map_or(Err(()), |_| Ok(true))
            }) {
                Some(Ok(b))  => Ok(b),
                Some(Err(_)) => Err(()),
                None         => Ok(false),
            }
        }
    }

    /// Move the cursor one step backward. `O(1)`.  
    /// Returns `false` if the cursor is at the edge,
    /// Returns `Err(())` if the cursor could not be moved.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// list.move_to_back();
    /// assert_eq!(list.get_cursor(), Some(&3));
    ///
    /// list.retreat();
    /// assert_eq!(list.get_cursor(), Some(&2));
    /// ```
    #[must_use]
    pub fn retreat(&self) -> Result<bool, ()> {
        unsafe {
            let current = self.current.load(Acquire);
            match current.as_ref().and_then(|c| c.prev.load_ptr(Acquire)).map(|prev| {
                self.index.fetch_sub(1, Release);
                self.current.compare_exchange(current, prev, Acquire, Relaxed).map_or(Err(()), |_| Ok(true))
            }) {
                Some(Ok(b))  => Ok(b),
                Some(Err(_)) => Err(()),
                None         => Ok(false),
            }
        }
    }

    /// Move the cursor by a given offset. `O(n)`.  
    /// If the offset is out of bounds the cursor will be moved to the edge, 
    /// and `false` will be returned.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// list.move_to_back();
    /// assert_eq!(list.index(), 2);
    ///
    /// list.move_by(-2);
    /// assert_eq!(list.index(), 0);
    /// assert_eq!(list.get_cursor(), Some(&1));
    ///
    /// assert!(!list.move_by(10).unwrap());
    /// assert_eq!(list.index(), 2);
    /// ```
    #[must_use]
    pub fn move_by(&self, offset: isize) -> Result<bool, ()> {
        match offset.cmp(&0) {
            Ordering::Greater => {
                for _ in 0..offset {
                    match self.advance() {
                        Ok(false) => return Ok(false),
                        Err(_)    => return Err(()),
                        _         => continue,
                    }
                }
            },
            Ordering::Less    => {
                for _ in 0..-offset {
                    match self.retreat() {
                        Ok(false) => return Ok(false),
                        Err(_)    => return Err(()),
                        _         => continue,
                    }
                }
            },
            _ => (),
        }

        Ok(true)
    }

    /// Get a ref to an element at the given offset. `O(n)`.  
    /// Returns `None` if the offset is out of bounds; ie. the value doesnt exist.  
    /// Returns `Err(())` if another thread modified the value during the function's run time.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// assert_eq!(list.get(1), Ok(Some(&2)));
    /// assert_eq!(list.get(-1), Ok(None));
    /// ```
    pub fn get(&self, offset: isize) -> Result<Option<&T>, ()> {
        let index = self.index.load(Relaxed);
        offset.checked_abs()
            .and_then(|s| (s > index as isize).into())
            .and_then(|_| (index as isize + offset > self.len.load(Relaxed) as isize).into()).ok_or(())?;

        let current = self.current.load_ptr(Acquire).ok_or(())?;
        let mut ptr = current;

        unsafe {
            match offset.cmp(&0) {
                Ordering::Greater => (0.. offset).for_each(|_| {ptr.as_ref().map(|c| ptr = c.next.load(Acquire)); }),
                Ordering::Less    => (0..-offset).for_each(|_| {ptr.as_ref().map(|c| ptr = c.prev.load(Acquire)); }),
                Ordering::Equal   => return Ok(self.get_cursor()),
            }

            if self.current.load(Acquire) != current { return Err(()); }
            Ok(ptr.as_ref().map(|c| &c.elem ))
        }
    }

    /// Get a mut ref to an element at the given offset. `O(n)`.  
    /// Returns `None` if the offset is out of bounds; ie. the value doesnt exist.  
    /// Returns `Err(())` if another thread modified the value during the function's run time.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// *list.get_mut(1).unwrap().unwrap() = 4;
    /// assert_eq!(format!("{:?}", list), "[1, 4, 3]");
    /// ```
    pub fn get_mut(&self, offset: isize) -> Result<Option<&mut T>, ()> {
        let index = self.index.load(Relaxed);
        offset.checked_abs()
            .and_then(|s| (s > index as isize).into())
            .and_then(|_| (index as isize + offset > self.len.load(Relaxed) as isize).into()).ok_or(())?;

        let current = self.current.load_ptr(Acquire).ok_or(())?;
        let mut ptr = current;

        unsafe {
            match offset.cmp(&0) {
                Ordering::Greater => (0.. offset).for_each(|_| {ptr.as_ref().map(|c| ptr = c.next.load(Acquire)); }),
                Ordering::Less    => (0..-offset).for_each(|_| {ptr.as_ref().map(|c| ptr = c.prev.load(Acquire)); }),
                Ordering::Equal   => return Ok(self.get_cursor_mut()),
            }

            if self.current.load(Acquire) != current { return Err(()); }
            Ok(ptr.as_mut().map(|c| &mut c.elem ))
        }
    }


    /// Remove the current element and return it. `O(1)`.  
    /// The cursor will then point to the next element.  
    /// If the removed element was at the end of the list, the cursor will point to the previous
    /// element and `false` will be returned.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(list.consume_forward(), Some((1, true)));
    /// assert_eq!(&format!("{:?}", list), "[2, 3]");
    /// assert_eq!(list.get_cursor(), Some(&2));
    /// ```
    pub fn consume_forward(&mut self) -> Option<(T, bool)> {
        self.current.load_ptr(SeqCst).map(|node| unsafe {
            let node = Box::from_raw(node);
            self.len.fetch_sub(1, Release);

            node.prev.load_ptr(Acquire)
                .map(|prev| (*prev).next.store(node.next.load(Relaxed), Relaxed));

            match node.next.load_ptr(Acquire) {
                Some(next) => {
                    (*next).prev.store(node.prev.load(Relaxed), Release);
                    self.current.store(node.next.load(Relaxed), Release);
                    (node.elem, true)
                },
                None => {
                    self.current.store(node.prev.load(Relaxed), Release);
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
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    /// list.move_by(1);
    /// assert_eq!(list.consume_backward(), Some((2, true)));
    /// assert_eq!(&format!("{:?}", list), "[1, 3]");
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// ```
    pub fn consume_backward(&mut self) -> Option<(T, bool)> {
        self.current.load_ptr(SeqCst).map(|node| unsafe {
            let node = Box::from_raw(node);
            self.len.fetch_sub(1, Release);
            self.index.fetch_sub(1, Release);

            node.next.load_ptr(Acquire)
                .map(|next| (*next).prev.store(node.prev.load(Relaxed), Relaxed));

            match node.prev.load_ptr(Acquire) {
                Some(prev) => {
                    (*prev).next.store(node.next.load(Relaxed), Release);
                    self.current.store(node.prev.load(Relaxed), Release);
                    (node.elem, true)
                },
                None => {
                    self.current.store(node.next.load(Relaxed), Release);
                    (node.elem, false)
                }
            }
        })
    }

    /// Replace the current element with a new one. `O(1)`.  
    /// Returns the old element.  
    /// If the list is empty, the new element will be inserted, and `None` returned.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(list.replace_cursor(4), Ok(Some(1)));
    /// assert_eq!(format!("{:?}", list), "[4, 2, 3]");
    /// ```
    #[inline]
    pub fn replace_cursor(&mut self, elem: T) -> Result<Option<T>, T> {
        unsafe {
            match self.current.load_ptr(SeqCst) {
                None => { self.insert_next(elem)?; Ok(None) }, 
                Some(node) => Ok(Some(std::mem::replace(&mut (*node).elem, elem))),
            }
        }
    }

    /// Split the list after the cursor. `O(1)`.  
    /// If the list is empty, or the cursor is at the end, `None` will be returned.  
    /// ```
    /// # use iterlist::atomic::IterList;
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
            self.current.load_ptr(SeqCst).and_then(|c| (*c).next.load_ptr(Acquire)).map(|next| {
                let mut new = Self::new();

                new.current = next.into();

                new.len = (self.len.load(Relaxed) - self.index.load(Relaxed) - 1).into();
                self.len.fetch_sub(new.len.load(Relaxed), Release);

                (*self.current.load(Acquire)).next.store(ptr::null_mut(), Release);
                (*new .current.load(Relaxed)).prev.store(ptr::null_mut(), Relaxed);

                new
            })
        }
    }

    /// Split the list before the cursor. `O(1)`.
    /// If the list is empty, or the cursor is at the front, `None` will be returned.
    /// ```
    /// # use iterlist::atomic::IterList;
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
            self.current.load_ptr(SeqCst).and_then(|c| (*c).prev.load_ptr(Acquire)).map(|prev| {
                let mut new = Self::new();

                new.current = prev.into();
                new.len     = self.index.load(Relaxed).into();
                self.len.fetch_sub(new.len.load(Relaxed), Release);
                self.index.store(0, Release);
                new.index   = (new.len.load(Relaxed) - 1).into();

                (*self.current.load(Acquire)).prev.store(ptr::null_mut(), Release);
                (*new .current.load(Relaxed)).next.store(ptr::null_mut(), Relaxed);

                new
            })
        }
    }

    /// Get a ref to the current element. `O(1)`.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// ```
    #[inline]
    pub fn get_cursor(&self) -> Option<&T> {
        unsafe { self.current.load_ptr(Acquire).map(|c| &(*c).elem ) }
    }

    /// Get a mut ref to the current element. `O(1)`.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// *list.get_cursor_mut().unwrap() = 4;
    /// assert_eq!(format!("{:?}", list), "[4, 2, 3]");
    /// ```
    #[inline]
    pub fn get_cursor_mut(&self) -> Option<&mut T> {
        unsafe { self.current.load_ptr(Acquire).map(|c| &mut (*c).elem ) }
    }

    /// Get the number of elements in the list. `O(1)`.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(list.len(), 3);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.len.load(Relaxed)
    }

    /// Check if the list is empty. `O(1)`.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let list: IterList<u8> = IterList::new();
    /// assert!(list.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.current.load(Relaxed).is_null()
    }

    /// Get the index of the cursor `O(1)`.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(list.index(), 0);
    /// ```
    #[inline]
    pub fn index(&self) -> usize {
        self.index.load(Relaxed)
    }

    /// Provides a copy of the current cursor. `O(1)`.
    /// ```
    /// # use iterlist::atomic::IterList;
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
            _list:   PhantomData,
            index:   self.index.load(Relaxed).into(),
            current: self.current.load(Relaxed).into(),
        }
    }
}




impl<T: Debug> Debug for IterList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[")?;

        let base = -(self.index() as isize);

        let mut i: isize = 0;
        while i < (self.len() as isize) {
            let element = self.get(base + i);

            if let Ok(Some(element)) = element {
                write!(f, "{:?}", element)?;
            }

            i += 1;

            if i < (self.len() as isize) {
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
            while let Some(current) = self.current.load_ptr(SeqCst) {
                self.current.store((*current).next.load(Relaxed), Relaxed);
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
    /// # use iterlist::atomic::IterList;
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
    /// # use iterlist::atomic::IterList;
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
    /// # use iterlist::atomic::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(format!("{:?}", list), "[1, 2, 3]");
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// ```
    fn from(vec: Vec<T>) -> Self {
        let list = vec.into_iter().fold(Self::new(), |list, elem| {
            unsafe { list.push_next(elem).unwrap_unchecked(); }
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
    /// # use iterlist::atomic::IterList;
    /// let array: &[u8] = &[1, 2, 3];
    /// let list = IterList::from(array);
    /// assert_eq!(format!("{:?}", list), "[1, 2, 3]");
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// ```
    fn from(slice: &[T]) -> Self {
        let list = slice.iter().cloned().fold(Self::new(), |list, elem| {
            unsafe { list.push_next(elem).unwrap_unchecked(); }
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
    /// # use iterlist::atomic::IterList;
    /// let list = (1..=3).into_iter().collect::<IterList<_>>();
    /// assert_eq!(format!("{:?}", list), "[1, 2, 3]");
    /// assert_eq!(list.get_cursor(), Some(&1));
    /// ```
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let list = iter.into_iter().fold(Self::new(), |list, elem| {
            unsafe { list.push_next(elem).unwrap_unchecked(); }
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
// #[derive(Clone, Copy)]
pub struct Cursor<'i, T> {
    current: AtomicPtr<Node<T>>,
    index:   AtomicUsize,
    _list:   PhantomData<&'i T>,
}

unsafe impl<'i, T: Send> Send for Cursor<'i, T> {}
unsafe impl<'i, T: Sync> Sync for Cursor<'i, T> {}

impl<'i, T> Iterator for Cursor<'i, T> {
    type Item = &'i T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            self.current.load_ptr(Acquire).map(|c| {
                self.current.store((*c).next.load(Relaxed), Release);
                self.index.fetch_add(1, AcqRel);
                &(*c).elem
            })
        }
    }
}

impl<'t, T: Send + Sync> Cursor<'t, T> {
    /// Get a mut ref to the current element. `O(1)`.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// let cursor = list.as_cursor();
    ///
    /// *cursor.get_cursor_mut().unwrap() = 4;
    /// assert_eq!(format!("{:?}", list), "[4, 2, 3]");
    /// ```
    #[inline]
    pub fn get_cursor_mut(&self) -> Option<&mut T> {
        unsafe { self.current.load_ptr(Acquire).map(|c| &mut (*c).elem ) }
    }


    /// Get a mut ref to an element at the given offset. `O(n)`.
    /// Returns `None` if the offset is out of bounds, or if another thread modified the value
    /// during the function's run time.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// let cursor = list.as_cursor();
    ///
    /// *cursor.get_mut(1).unwrap().unwrap() = 4;
    /// assert_eq!(format!("{:?}", list), "[1, 4, 3]");
    /// ```
    pub fn get_mut(&self, offset: isize) -> Result<Option<&mut T>, ()> {
        offset.checked_abs().and_then(|s| (s > self.index.load(Relaxed) as isize).into()).ok_or(())?;

        let current = self.current.load_ptr(Acquire).ok_or(())?;
        let mut ptr = current;

        unsafe {
            match offset.cmp(&0) {
                Ordering::Greater => (0.. offset).for_each(|_| {ptr.as_ref().map(|c| ptr = c.next.load(Acquire)); }),
                Ordering::Less    => (0..-offset).for_each(|_| {ptr.as_ref().map(|c| ptr = c.prev.load(Acquire)); }),
                Ordering::Equal   => return Ok(self.get_cursor_mut()),
            }

            if self.current.load(Acquire) != current { return Err(()); }
            Ok(ptr.as_mut().map(|c| &mut c.elem ))
        }
    }
}

impl<'t, T> Cursor<'t, T> {
    /// Update the cursor to match the current state of the list. `O(1)`.  
    /// Useful if you lose track of the list, or want to use the same cursor on multiple lists.
    #[inline]
    pub fn reacquire(&mut self, list: &'t IterList<T>) {
        self.current = list.current.load(Relaxed).into();
        self.index   = list.index.load(Relaxed).into();
    }

    /// Get a ref to the current element. `O(1)`.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// let mut cursor = list.as_cursor();
    ///
    /// assert_eq!(cursor.get_cursor(), Some(&1));
    /// ```
    #[inline]
    pub fn get_cursor(&self) -> Option<&T> {
        unsafe { self.current.load_ptr(Acquire).map(|c| &(*c).elem ) }
    }

    /// Get the index of the cursor `O(1)`.
    /// ```
    /// # use iterlist::atomic::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// let mut cursor = list.as_cursor();
    ///
    /// cursor.next();
    /// assert_eq!(cursor.index(), 1);
    /// ```
    #[inline]
    pub fn index(&self) -> usize {
        self.index.load(Relaxed)
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
    pub fn move_to_front(&self) -> usize {
        unsafe {
            self.index.store(0, Relaxed);
            
            let mut offset = 0;
            while let Some(prev) = self.current.load(Acquire).as_ref()
                .map(|c| c.prev.load(Acquire)).filter(|p| !p.is_null()) 
            {
                self.current.store(prev, Release);
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
    pub fn move_to_back(&self) -> usize {
        unsafe {
            let mut offset = 0;
            while let Some(next) = self.current.load(Acquire).as_ref()
                .map(|c| c.next.load(Acquire)).filter(|p| !p.is_null()) 
            {
                self.current.store(next, Release);
                offset += 1;
            }
            self.index.store(offset, Release);
            offset
        }
    }

    /// Move the cursor to the specified index. `O(n)`.  
    /// Returns the number of elements traversed. It is up to the user to check if that number is correct.  
    /// Returns `Err(n)` if the cursor could not be moved at any point  
    /// ```
    /// # use iterlist::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// let mut cursor = list.as_cursor();
    ///
    /// cursor.move_to(1);
    /// assert_eq!(cursor.get_cursor(), Some(&2));
    /// ```
    #[must_use]
    pub fn move_to(&self, index: usize) -> Result<usize, usize> {
        match self.index.load(Relaxed).cmp(&index) {
            Ordering::Greater => {
                for i in 0..index - self.index.load(Relaxed) {
                    match self.retreat() {
                        Ok(false) => return Ok(i),
                        Err(_)    => return Err(i),
                        _         => continue,
                    }
                }
            },
            Ordering::Less    => {
                for i in 0..index - self.index.load(Relaxed) {
                    match self.advance() {
                        Ok(false) => return Ok(i),
                        Err(_)    => return Err(i),
                        _         => continue,
                    }
                }
            },
            _ => (),
        }
        Ok(index)
    }


    /// Move the cursor one step forward. `O(1)`.  
    /// Returns `false` if the cursor is at the edge.
    /// Returns `Err(())` if the cursor could not be moved.
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
    #[must_use]
    pub fn advance(&self) -> Result<bool, ()> {
        unsafe {
            let current = self.current.load(Acquire);
            match current.as_ref().and_then(|c| c.next.load_ptr(Acquire)).map(|c| {
                self.index.fetch_add(1, Release);
                self.current.compare_exchange(current, c, Acquire, Relaxed).map_or(Err(()), |_| Ok(true))
            }) {
                Some(Ok(b))  => Ok(b),
                Some(Err(_)) => Err(()),
                None         => Ok(false),
            }
        }
    }

    /// Move the cursor one step backward. `O(1)`.  
    /// Returns `false` if the cursor is at the edge,
    /// Returns `Err(())` if the cursor could not be moved.
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
    #[must_use]
    pub fn retreat(&self) -> Result<bool, ()> {
        unsafe {
            let current = self.current.load(Acquire);
            match current.as_ref().and_then(|c| c.prev.load_ptr(Acquire)).map(|prev| {
                self.index.fetch_sub(1, Release);
                self.current.compare_exchange(current, prev, Acquire, Relaxed).map_or(Err(()), |_| Ok(true))
            }) {
                Some(Ok(b))  => Ok(b),
                Some(Err(_)) => Err(()),
                None         => Ok(false),
            }
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
    #[must_use]
    pub fn move_by(&self, offset: isize) -> Result<bool, ()> {
        match offset.cmp(&0) {
            Ordering::Greater => {
                for _ in 0..offset {
                    match self.advance() {
                        Ok(false) => return Ok(false),
                        Err(_)    => return Err(()),
                        _         => continue,
                    }
                }
            },
            Ordering::Less    => {
                for _ in 0..-offset {
                    match self.retreat() {
                        Ok(false) => return Ok(false),
                        Err(_)    => return Err(()),
                        _         => continue,
                    }
                }
            },
            _ => (),
        }

        Ok(true)
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
    pub fn get(&self, offset: isize) -> Result<Option<&T>, ()> {
        offset.checked_abs().and_then(|s| (s > self.index.load(Relaxed) as isize).into()).ok_or(())?;

        let current = self.current.load_ptr(Acquire).ok_or(())?;
        let mut ptr = current;

        unsafe {
            match offset.cmp(&0) {
                Ordering::Greater => (0.. offset).for_each(|_| {ptr.as_ref().map(|c| ptr = c.next.load(Acquire)); }),
                Ordering::Less    => (0..-offset).for_each(|_| {ptr.as_ref().map(|c| ptr = c.prev.load(Acquire)); }),
                Ordering::Equal   => return Ok(self.get_cursor()),
            }

            if self.current.load(Acquire) != current { return Err(()); }
            Ok(ptr.as_ref().map(|c| &c.elem ))
        }
    }
}

impl<'i, T> std::ops::Deref for Cursor<'i, T> {
    type Target = T;

    /// Essentially equivalent to `get_cursor`. `O(1)`.
    /// # Panics
    /// Panics if the cursor is out of bounds.
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.get_cursor().unwrap_or_else(|| panic!("Cursor out of bounds"))
    }
}


impl<'i, T> std::ops::Index<isize> for Cursor<'i, T> {
    type Output = T;

    /// Essentially equivalent to `get`. `O(n)`.  
    /// # Panics
    /// Panics if the index is out of bounds.
    #[inline]
    fn index(&self, index: isize) -> &Self::Output {
        self.get(index)
            .unwrap_or_else(|_| panic!("Index out of bounds"))
            .unwrap_or_else(| | panic!("Index out of bounds"))
    }
}

impl<T: Debug> Debug for Cursor<'_, T> {
    /// ```
    /// # use iterlist::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// let mut cursor = list.as_cursor();
    /// assert_eq!(format!("{:?}", cursor), "0: Some(1)");
    /// ```
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}: {:?}", self.index.load(Relaxed), self.get_cursor())
    }
}




pub trait PtrExt<T> {
    fn load_ptr(&self, order: std::sync::atomic::Ordering) -> Option<*mut T>;
}

impl<T> PtrExt<T> for AtomicPtr<T> {
    #[inline]
    fn load_ptr(&self, order: std::sync::atomic::Ordering) -> Option<*mut T> {
        let ptr = self.load(order);
        (!ptr.is_null()).then_some(ptr) 
    }
}
