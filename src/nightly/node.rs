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
			NonNull::new_unchecked(Box::into_raw(Box::new(Self {
				next: None,
				prev: None,
				elem,
			})))
		}
	}
}

// TODO: one day use specialization
// this is needlessly slow for T: Copy as ptr::copy_nonoverlapping is one op
impl<T: Clone> NodeTrait<&[T], [T]> for Node<[T]> {
	fn new_nonnull(elem: &[T]) -> NonNull<Self> {
		unsafe {
			let node = NonNull::new_unchecked(ptr::slice_from_raw_parts_mut(
				alloc::alloc(alloc::Layout::from_size_align(
					mem::size_of::<Node<()>>() + mem::size_of_val(elem), 8)
					.unwrap_unchecked()) as *mut T, elem.len()) as *mut Node<[T]>);

// 			ptr::copy_nonoverlapping(
// 				elem.as_ptr(), (*node.as_ptr()).elem.as_mut_ptr(), elem.len());
			elem.iter().zip((*node.as_ptr()).elem.iter_mut())
				.for_each(|(elem, node)| *node = elem.clone());

			node
		}
	}
}

impl NodeTrait<&str, str> for Node<str> {
	fn new_nonnull(elem: &str) -> NonNull<Self> {
		unsafe { mem::transmute(Node::<[u8]>::new_nonnull(elem.as_bytes())) }
	}
}
