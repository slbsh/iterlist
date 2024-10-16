# Iter Who? IterList!

It's a doubly linked list with a cursor based api.  
*also an iterator!*  

`O(1)` pretty much everything (at and around the cursor).  
Originally made it for [Shard](https://github.com/shard-org/shard), but thought it could be useful to someone else.  

Sign here to join the linked list uprising against the `Vec` tyranny!  
- [x] 

**Now Featuring:** the `atomic` module!  
Are you bored of performant data structures?  
Do you want to do some lock-free shenanigans?  
*slaps hood*  
Well this baby's now `Send + Sync` and can mutate atomically across threads!  

~~Using Result<Option<Result<Result ...~~   
![Hollow](./hollow.png)

## Example

```rust
use iterlist::IterList;

let mut list = IterList::new();

list.push_prev(-1);
list.push_next(1);
list.push_next(2);
list.push_next(3);

assert_eq!(format!("{:?}", list), "[-1, 1, 2, 3]");

list.move_to(2);
assert_eq!(list.get_cursor(), Some(&2));

list.move_by(-2);
assert_eq!(list.index(), 0);

let mut cursor = list.as_cursor();
assert_eq!(cursor.next(), Some(&-1));
assert_eq!(cursor.next(), Some(&1));

assert_eq!(list.get(1), Some(&1));

list.move_by(2);
let (elem, _) = list.consume_forward().unwrap();
assert_eq!(elem, 2);

assert_eq!(format!("{:?}", list), "[-1, 1, 3]");

let num = list.fold(0, |acc, elem| acc + elem);

assert_eq!(num, 3);
```

## Why would I want to use `IterList`?
- You're iterating over a list, and are removing/inserting elements as you go. 
    (In my tests it was marginally better than `std::collections::VecDeque`)
- You want to have multiple independent cursors on the same list.
- You need an iterator that you can move around in and modify.
- It's also noticably faster than `std::collections::LinkedList` in most cases!
- You wanna look cool (⌐■-■) ... or not, I'm not your boss. (psst the api is preeetty nice :p)

## Todos
- [ ] `append`  - append another list to the end of this one.
- [ ] `prepend` - prepend another list to the start of this one.
- [ ] `drain`   - remove a range of elements (around the cursor) from the list.
- [ ] `splice`  - replace a range of elements (around the cursor) with another list.
- [ ] `DoubleEndedIterator` for `Cursor`.
- [x] `feature(atomic)` - atomic IterList and Cursor.
- [ ] `feature(pool)` - semi-pool allocated list for grouping elements into contiguous memory.
- [ ] `feature(no_std)` - no std support.

Feel free to add any of these if ya wanna!
