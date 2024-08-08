# IterList

A doubly linked list with a cursor based api.  
*it's also an iterator!*  

`O(1)` pretty much everything (at the cursor).  

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
list.consume();

assert_eq!(format!("{:?}", list), "[-1, 1, 3]");

let num = list.fold(0, |acc, elem| acc + elem);

assert_eq!(num, 3);
```

## Why would I want to use `IterList`?
Short answer? *idk honestly.*  
Long answer:
- You're iterating over a list, and are removing/inserting elements as you go.
- You want to have multiple independant cursors on the same list.
- You need an iterator that you can move around in and modify.

## Why wouldn't I want to use `IterList`?
Pretty much any other case. (*lol*)  
It has all the disadvantages of a doubly linked list, and is even slower at many front/back operations.
Instead of pointers to front and back, `IterList` keeps the cursor and it's index. Meaning O(1) operations work only around the cursor.  

<br>

## Todos
- [ ] `replace` - replace the element at the cursor with another.
- [ ] `split`   - split the list at the cursor.
- [ ] `append`  - append another list to the end of this one.
- [ ] `prepend` - prepend another list to the start of this one.
- [ ] `drain`   - remove a range of elements (around the cursor) from the list.
- [ ] `splice`  - replace a range of elements (around the cursor) with another list.
- [ ] `DoubleEndedIterator` for `Cursor`.

If ya wanna add any of these, feel free to!  
