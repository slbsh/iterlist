use std::fmt::Debug;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::cmp::Ordering;

pub struct IterList<T> {
    current: Link<T>,
    offset:  usize,
    len:     usize,
    _boo:    PhantomData<T>,
}

type Link<T> = Option<NonNull<Node<T>>>;

#[derive(Debug, Clone)]
struct Node<T> {
    next: Link<T>,
    prev: Link<T>,
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
        Self { current: None, len: 0, offset: 0, _boo: PhantomData }
    }

    /// Insert an element after the cursor. `O(1)`.
    /// If the cursor is not at any element, it will be inserted at current position.
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
        unsafe {
            let new = NonNull::new_unchecked(Box::into_raw(Box::new(Node {
                prev: None,
                next: None,
                elem,
            })));

            
            if let Some(next) = self.current.and_then(|node| node.as_ref().next) {
                (*next.as_ptr()).prev = Some(new);
                (*new.as_ptr()).next = Some(next);
            }

            match self.current {
                Some(current) => {
                    (*current.as_ptr()).next = Some(new);
                    (*new.as_ptr()).prev = Some(current);
                },
                None => self.current = Some(new),
            }
            self.len += 1;
        }
    }

    /// Insert an element before the cursor. `O(1)`.
    /// If the cursor is not at any element, it will be inserted at current position.
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
        unsafe {
            let new = NonNull::new_unchecked(Box::into_raw(Box::new(Node {
                prev: None,
                next: None,
                elem,
            })));

            if let Some(prev) = self.current.and_then(|node| node.as_ref().prev) {
                (*prev.as_ptr()).next = Some(new);
                (*new.as_ptr()).prev = Some(prev);
            }

            match self.current {
                Some(old) => {
                    (*old.as_ptr()).prev = Some(new);
                    (*new.as_ptr()).next = Some(old);
                    self.offset += 1;
                },
                None => self.current = Some(new),
            }
            self.len += 1;
        }
    }

    /// Push an element after the cursor, moving the cursor to it. `O(1)`.
    /// If the cursor is not at any element, it will be inserted at current position.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::new();
    /// list.push_next(1);
    /// list.push_next(2);
    /// list.push_next(3);
    ///
    /// assert_eq!(list.current(), Some(&3));
    /// assert_eq!(&format!("{:?}", list), "[1, 2, 3]");
    /// ```
    pub fn push_next(&mut self, elem: T) {
        match self.current {
            Some(_) => {
                self.insert_next(elem);
                self.advance();
            },
            None => self.insert_next(elem),
        }
    }

    /// Push an element before the cursor, moving the cursor to it. `O(1)`.
    /// If the cursor is not at any element, it will be inserted at current position.
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
        match self.current {
            Some(_) => {
                self.insert_prev(elem);
                self.retreat();
            },
            None => self.insert_prev(elem),
        }
    }

    /// Move the cursor to the front of the list. `O(n)`.
    /// Returns the number of elements traversed.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// let offset = list.goto_front();
    /// assert_eq!(offset, 0);
    /// assert_eq!(list.current(), Some(&1));
    /// ```
    pub fn goto_front(&mut self) -> usize {
        self.offset = 0;
        unsafe {
            let mut offset = 0;
            while let Some(node) = self.current.as_ref().and_then(|node| node.as_ref().prev) {
                self.current = Some(node);
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
    /// let offset = list.goto_back();
    /// assert_eq!(offset, 2);
    /// assert_eq!(list.current(), Some(&3));
    /// ```
    pub fn goto_back(&mut self) -> usize {
        self.offset = self.len - 1;
        unsafe {
            let mut offset = 0;
            while let Some(node) = self.current.as_ref().and_then(|node| node.as_ref().next) {
                self.current = Some(node);
                offset += 1;
            }
            offset
        }
    }

    /// Move the cursor to the specified index. `O(n)`.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    /// list.goto(1);
    /// assert_eq!(list.current(), Some(&2));
    /// ```
    pub fn goto(&mut self, index: usize) {
        if index > self.len {
            panic!("Index out of bounds");
        }

        match self.offset.cmp(&index) {
            Ordering::Greater => (0..self.offset - index).for_each(|_| self.retreat()),
            Ordering::Less    => (0..index - self.offset).for_each(|_| self.advance()),
            Ordering::Equal => (),
        }
    }

    /// Move the cursor one step forward. `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(list.current(), Some(&1));
    ///
    /// list.advance();
    /// assert_eq!(list.current(), Some(&2));
    /// ```
    #[inline]
    pub fn advance(&mut self) {
        self.current.map(|node| unsafe {
            self.current = node.as_ref().next;
            self.offset += 1;
        });
    }

    /// Move the cursor one step backward. `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// list.goto_back();
    /// assert_eq!(list.current(), Some(&3));
    ///
    /// list.retreat();
    /// assert_eq!(list.current(), Some(&2));
    /// ```
    #[inline]
    pub fn retreat(&mut self) {
        self.current.map(|node| unsafe {
            self.current = node.as_ref().prev;
            self.offset -= 1;
        });
    }

    /// Move the cursor by a given offset. `O(n)`.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// list.move_by(-2);
    /// assert_eq!(list.current(), Some(&1));
    /// ```
    pub fn move_by(&mut self, offset: isize) {
        self.offset = (self.offset as isize).saturating_add(offset) as usize;
        match offset.cmp(&0) {
            Ordering::Greater => (0..offset).for_each(|_| self.advance()),
            Ordering::Less    => (0..offset).for_each(|_| self.retreat()),
            Ordering::Equal   => (),
        }
    }

    /// Get a ref to an element at the given offset. `O(n)`.
    /// ```
    /// # use iterlist::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(list.get(1), Some(&2));
    /// assert_eq!(list.get(-1), None);
    /// ```
    pub fn get(&self, offset: isize) -> Option<&T> {
        let mut current = self.current;

        unsafe {
            match offset.cmp(&0) {
                Ordering::Greater => (0..offset).for_each(|_| current = current.and_then(|node| node.as_ref().next)),
                Ordering::Less => (0..-offset).for_each(|_| current = current.and_then(|node| node.as_ref().prev)),
                Ordering::Equal => return self.current(),
            }

            current.map(|node| &node.as_ref().elem )
        }
    }

    /// Get a mut ref to an element at the given offset. `O(n)`.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// *list.get_mut(1).unwrap() = 4;
    /// assert_eq!(format!("{:?}", list), "[1, 4, 3]");
    /// ```
    pub fn get_mut(&mut self, offset: isize) -> Option<&mut T> {
        let mut current = self.current;

        unsafe {
            match offset.cmp(&0) {
                Ordering::Greater => (0..offset).for_each(|_| current = current.and_then(|node| node.as_ref().next)),
                Ordering::Less => (0..-offset).for_each(|_| current = current.and_then(|node| node.as_ref().prev)),
                Ordering::Equal => return self.current.map(|mut elem| &mut elem.as_mut().elem),
            }

            current.map(|mut node| &mut node.as_mut().elem )
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
        self.current.map(|node| unsafe {
            let node = Box::from_raw(node.as_ptr());

            if let Some(prev) = node.prev {
                (*prev.as_ptr()).next = node.next
            }

            if let Some(next) = node.next {
                (*next.as_ptr()).prev = node.prev
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
    /// assert_eq!(list.current(), Some(&1));
    /// ```
    #[inline]
    pub fn current(&self) -> Option<&T> {
        self.current.map(|node| unsafe { &node.as_ref().elem })
    }

    /// Get a mut ref to the current element. `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let mut list = IterList::from(vec![1, 2, 3]);
    ///
    /// *list.current_mut().unwrap() = 4;
    /// assert_eq!(format!("{:?}", list), "[4, 2, 3]");
    /// ```
    #[inline]
    pub fn current_mut(&mut self) -> Option<&mut T> {
        self.current.map(|mut node| unsafe { &mut node.as_mut().elem })
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
    /// let list = IterList::new();
    /// assert!(list.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the offset of the current element. `O(1)`.
    /// ```
    /// # use iterlist::IterList;
    /// let list = IterList::from(vec![1, 2, 3]);
    /// assert_eq!(list.offset(), 0);
    /// ```
    #[inline]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Get a raw pointer to the front element. `O(n)`.
    fn raw_front(&self) -> Option<NonNull<Node<T>>> {
        unsafe {
            let mut current = self.current;
            while let Some(node) = current.as_ref().and_then(|node| node.as_ref().prev) {
                current = Some(node);
            }
            current
        }
    }
}

impl<T> std::ops::Index<isize> for IterList<T> {
    type Output = T;

    fn index(&self, index: isize) -> &Self::Output {
        if index.checked_abs().is_some_and(|a| a > self.len() as isize) {
            panic!("Index out of bounds");
        }

        self.get(index).unwrap()
    }
}

impl<T> Default for IterList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> std::ops::IndexMut<isize> for IterList<T> {
    fn index_mut(&mut self, index: isize) -> &mut Self::Output {
        if index.checked_abs().is_some_and(|a| a > self.len() as isize) {
            panic!("Index out of bounds");
        }

        self.get_mut(index).unwrap()
    }
}

impl<T: Clone> Clone for IterList<T> {
    fn clone(&self) -> Self {
        unsafe {
            let mut current = self.current;

            let mut offset = 0; 
            while let Some(node) = current.as_ref().and_then(|node| node.as_ref().prev) {
                current = Some(node);
                offset += 1;
            }

            let mut list = Self::new();

            while let Some(node) = current.as_ref().and_then(|node| node.as_ref().next) {
                current = Some(node);
                list.push_next(node.as_ref().elem.clone());
            }

            for _ in 0..offset {
                current = current.as_ref().and_then(|node| node.as_ref().prev);
            }

            list.current = current;
            list
        }
    }
}

impl<T> From<Vec<T>> for IterList<T> {
    /// Create a new list from a vector. `O(n)`.
    /// Cursor is set to the front of the list.
    fn from(vec: Vec<T>) -> Self {
        let mut list = vec.into_iter().fold(Self::new(), |mut list, elem| {
            list.push_next(elem);
            list
        });
        list.goto_front();
        list
    }
}

impl<T: Debug> Debug for IterList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[")?;

        let mut current = self.raw_front();
        while let Some(n) = current {
            write!(f, "{:?}", unsafe{&n.as_ref().elem})?;
            unsafe { current = current.as_ref().and_then(|node| node.as_ref().next) }
            if current.is_some() {
                write!(f, ", ")?;
            }
        }

        write!(f, "]")
    }
}
