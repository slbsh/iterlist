#[macro_export]
macro_rules! list {
	() => ( iterlist::IterList::new() );
	($elem:expr) => ({
		let mut list = iterlist::IterList::new();
		unsafe { list.insert_init($elem); }
		list
	});
	($elem:expr; $n:expr) => ( iterlist::IterList::from_elem($elem, $n) );
	($($x:expr),+ $(,)?) => ({
		let mut list = iterlist::IterList::new();
		$(list.push_next($x);)+
		list.goto_start();
		list
	});
}
