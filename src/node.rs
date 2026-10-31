use std::ptr::{self, NonNull};
use std::alloc;
use std::mem;

pub struct Node<T: ?Sized> {
	pub prev: Option<NonNull<Node<T>>>,
	pub next: Option<NonNull<Node<T>>>,
	pub elem: T,
}

pub trait NodeTrait<F: ?Sized, T: ?Sized> {
	fn new_nonnull(elem: F) -> NonNull<Node<T>>;
}

impl<T: Sized> NodeTrait<T, T> for Node<T> {
	fn new_nonnull(elem: T) -> NonNull<Self> {
		unsafe {
			Box::into_non_null(Box::new(Self {
				next: None,
				prev: None,
				elem,
			}))
		}
	}
}

impl<T: Clone> NodeTrait<&[T], [T]> for Node<[T]> {
	fn new_nonnull(elem: &[T]) -> NonNull<Self> {
		unsafe {
			let (layout, offset) = alloc::Layout::new::<Option<NonNull<Node<[T]>>>>()
				.repeat(2).unwrap().0.extend(alloc::Layout::for_value(elem)).unwrap();

			let mut node = NonNull::new(
				std::ptr::slice_from_raw_parts_mut(
					alloc::alloc_zeroed(layout.pad_to_align()).cast::<T>(),
					elem.len()) as *mut Node<[T]>).expect("alloc fail");

			(&raw mut (*node.as_ptr()).next).write(None);
			(&raw mut (*node.as_ptr()).prev).write(None);

			elem.iter().enumerate().for_each(|(i, e)|
				node.as_ptr().byte_add(offset).cast::<T>().add(i).write(e.clone()));

			node
		}
	}
}

// TODO: one day use specialization
// impl<T: Copy> NodeTrait<&[T], [T]> for Node<[T]> {
// 	fn new_nonnull(elem: &[T]) -> NonNull<Self> {
// 		unsafe {
// 			let (layout, offset) = alloc::Layout::new::<Option<NonNull<Node<[T]>>>>()
// 				.repeat(2).unwrap().0.extend(alloc::Layout::for_value(elem)).unwrap();
//
// 			let mut node = NonNull::new(
// 				std::ptr::slice_from_raw_parts_mut(
// 					alloc::alloc_zeroed(layout.pad_to_align()).cast::<T>(),
// 					elem.len()) as *mut Node<[T]>).expect("alloc fail");
//
// 			(&raw mut (*node.as_ptr()).next).write(None);
// 			(&raw mut (*node.as_ptr()).prev).write(None);
//
// 			ptr::copy_nonoverlapping(
// 				elem.as_ptr(), node.as_ptr().byte_add(offset).cast::<T>(), elem.len());
//
// 			node
// 		}
// 	}
// }

impl NodeTrait<&str, str> for Node<str> {
	fn new_nonnull(elem: &str) -> NonNull<Self> {
		unsafe { mem::transmute(Node::<[u8]>::new_nonnull(elem.as_bytes())) }
	}
}
