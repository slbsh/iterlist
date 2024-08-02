use std::fmt::Debug;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::cmp::Ordering;

pub struct IterList<T> {
    current: Link<T>,
    len:     usize,
    _boo:    PhantomData<T>,
}

type Link<T> = Option<NonNull<Node<T>>>;

#[derive(Debug, Clone)]
pub struct Node<T> {
    next: Link<T>,
    prev: Link<T>,
    elem: T,
}

impl<T> IterList<T> {
    pub fn new() -> Self {
        Self { current: None, len: 0, _boo: PhantomData }
    }

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
                },
                None => self.current = Some(new),
            }
            self.len += 1;
        }
    }

    pub fn push_next(&mut self, elem: T) {
        match self.current {
            Some(_) => {
                self.insert_next(elem);
                self.advance();
            },
            None => self.insert_next(elem),
        }
    }

    pub fn push_prev(&mut self, elem: T) {
        match self.current {
            Some(_) => {
                self.insert_prev(elem);
                self.retreat();
            },
            None => self.insert_prev(elem),
        }
    }

    pub fn goto_front(&mut self) -> usize {
        unsafe {
            let mut offset = 0;
            while let Some(node) = self.current.as_ref().and_then(|node| node.as_ref().prev) {
                self.current = Some(node);
                offset += 1;
            }
            offset
        }
    }

    pub fn goto_back(&mut self) -> usize {
        unsafe {
            let mut offset = 0;
            while let Some(node) = self.current.as_ref().and_then(|node| node.as_ref().next) {
                self.current = Some(node);
                offset += 1;
            }
            offset
        }
    }

    pub fn goto(&mut self, index: usize) {
        if index > self.len {
            panic!("Index out of bounds");
        }

        self.goto_front();
        for _ in 0..index {
            self.advance();
        }
    }

    pub fn advance(&mut self) {
        self.current.map(|node| unsafe {
            self.current = node.as_ref().next;
        });
    }

    pub fn retreat(&mut self) {
        self.current.map(|node| unsafe {
            self.current = node.as_ref().prev;
        });
    }

    pub fn current(&self) -> Option<&T> {
        self.current.map(|node| unsafe { &node.as_ref().elem })
    }

    pub fn peek(&self) -> Option<&T> {
        self.get_offset(1)
    }

    pub fn offset(&mut self, offset: isize) {
        match offset.cmp(&0) {
            Ordering::Greater => {
                for _ in 0..offset {
                    self.advance();
                }
            },
            Ordering::Less => {
                for _ in 0..-offset {
                    self.retreat();
                }
            },
            Ordering::Equal => (),
        }
    }

    pub fn get_offset(&self, offset: isize) -> Option<&T> {
        let mut current = self.current.clone();

        unsafe {
            match offset.cmp(&0) {
                Ordering::Greater => (0..offset).for_each(|_| current = current.and_then(|node| node.as_ref().next)),
                Ordering::Less => (0..-offset).for_each(|_| current = current.and_then(|node| node.as_ref().prev)),
                Ordering::Equal => return self.current(),
            }

            current.map(|node| &node.as_ref().elem )
        }
    }

    pub fn get_offset_mut(&mut self, offset: isize) -> Option<&mut T> {
        let mut current = self.current.clone();

        unsafe {
            match offset.cmp(&0) {
                Ordering::Greater => (0..offset).for_each(|_| current = current.and_then(|node| node.as_ref().next)),
                Ordering::Less => (0..-offset).for_each(|_| current = current.and_then(|node| node.as_ref().prev)),
                Ordering::Equal => return self.current.map(|mut elem| &mut elem.as_mut().elem),
            }

            current.map(|mut node| &mut node.as_mut().elem )
        }
    }

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

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn raw_front(&self) -> Option<NonNull<Node<T>>> {
        unsafe {
            let mut current = self.current;
            while let Some(node) = current.as_ref().and_then(|node| node.as_ref().prev) {
                current = Some(node);
            }
            current
        }
    }

    pub fn raw_back(&self) -> Option<NonNull<Node<T>>> {
        unsafe {
            let mut current = self.current;
            while let Some(node) = current.as_ref().and_then(|node| node.as_ref().next) {
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

        self.get_offset(index).unwrap()
    }
}

impl<T> std::ops::IndexMut<isize> for IterList<T> {
    fn index_mut(&mut self, index: isize) -> &mut Self::Output {
        if index.checked_abs().is_some_and(|a| a > self.len() as isize) {
            panic!("Index out of bounds");
        }

        self.get_offset_mut(index).unwrap()
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
