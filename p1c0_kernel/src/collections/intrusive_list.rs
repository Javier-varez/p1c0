use super::OwnedMutPtr;

#[derive(Debug)]
pub struct IntrusiveList<T> {
    head: *mut IntrusiveItem<T>,
    tail: *mut IntrusiveItem<T>,
    length: usize,
}

unsafe impl<T> Send for IntrusiveList<T> {}

impl<T> IntrusiveList<T> {
    pub const fn new() -> Self {
        Self {
            head: core::ptr::null_mut(),
            tail: core::ptr::null_mut(),
            length: 0,
        }
    }

    /// Appends an element to the tail of the queue
    pub fn push(&mut self, item: OwnedMutPtr<IntrusiveItem<T>>) {
        if self.head.is_null() {
            self.head = item.leak();
            self.tail = self.head;
            self.length = 1;
        } else {
            let new_item = item.leak();
            unsafe {
                (*self.tail).next = new_item;
                (*new_item).prev = self.tail;

                self.tail = new_item;
                self.length += 1;
            }
        }
    }

    /// Pops head and returns it if there are any objects in the queue
    pub fn pop(&mut self) -> Option<OwnedMutPtr<IntrusiveItem<T>>> {
        if self.head.is_null() {
            return None;
        }

        let item = self.head;
        self.head = unsafe { (*item).next };
        unsafe { (*item).next = core::ptr::null_mut() };

        if self.head.is_null() {
            self.tail = core::ptr::null_mut();
        } else {
            unsafe { (*self.head).prev = core::ptr::null_mut() };
        }

        self.length -= 1;
        let item = unsafe { OwnedMutPtr::new_from_raw(item) };
        Some(item)
    }

    pub fn iter(&self) -> IntrusiveListIter<'_, T> {
        if self.head.is_null() {
            IntrusiveListIter {
                head_item: None,
                tail_item: core::ptr::null(),
            }
        } else {
            IntrusiveListIter {
                head_item: Some(unsafe { &*self.head }),
                tail_item: self.tail,
            }
        }
    }

    pub fn iter_mut(&mut self) -> IntrusiveListMutIter<'_, T> {
        if self.head.is_null() {
            IntrusiveListMutIter {
                head_item: None,
                tail_item: core::ptr::null_mut(),
            }
        } else {
            IntrusiveListMutIter {
                head_item: Some(unsafe { &mut *self.head }),
                tail_item: self.tail,
            }
        }
    }

    fn remove_element(
        &mut self,
        element: *mut IntrusiveItem<T>,
    ) -> Option<OwnedMutPtr<IntrusiveItem<T>>> {
        if element.is_null() {
            return None;
        }

        let prev = unsafe { (*element).prev };
        let next = unsafe { (*element).next };

        if !prev.is_null() {
            unsafe { (*prev).next = next };
        }

        if !next.is_null() {
            unsafe { (*next).prev = prev };
        }

        // Check if the element we removed is head or tail (or both) and update them
        if element == self.head {
            self.head = next;
        }

        if element == self.tail {
            self.tail = prev;
        }

        let mut element = unsafe { OwnedMutPtr::new_from_raw(element) };
        element.next = core::ptr::null_mut();
        element.prev = core::ptr::null_mut();
        self.length -= 1;
        Some(element)
    }

    pub fn remove(&mut self, index: usize) -> Option<OwnedMutPtr<IntrusiveItem<T>>> {
        let mut element = self.head;
        for _i in 0..index {
            if element.is_null() {
                return None;
            }

            // Move to next
            element = unsafe { (*element).next };
        }

        self.remove_element(element)
    }

    pub fn drain_filter<F>(&mut self, mut filter: F) -> IntrusiveList<T>
    where
        F: FnMut(&mut T) -> bool,
    {
        let mut removed_entries = Self::new();

        let mut element = self.head;
        while !element.is_null() {
            let element_ref = unsafe { &mut (*element).inner };
            let next = unsafe { (*element).next };

            if filter(element_ref) {
                let removed_entry = self
                    .remove_element(element)
                    .expect("The element is not valid");
                removed_entries.push(removed_entry);
            }

            // Move to next element
            element = next;
        }

        removed_entries
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    pub fn len(&self) -> usize {
        self.length
    }

    /// Consumes the list and calls the given callable to free/return each element
    pub fn release<F>(mut self, mut free: F)
    where
        F: FnMut(OwnedMutPtr<IntrusiveItem<T>>),
    {
        let mut element = self.head;
        while !element.is_null() {
            let mut element_ref = unsafe { OwnedMutPtr::new_from_raw(element) };
            let next = element_ref.next;

            element_ref.next = core::ptr::null_mut();
            element_ref.prev = core::ptr::null_mut();

            free(element_ref);

            element = next;
        }

        // Since we went through the list in one way, remove the head and tail now
        // It is important that we do this because the list will be dropped later and we
        // need to make sure that the elements are actually released
        self.head = core::ptr::null_mut();
        self.tail = core::ptr::null_mut();
    }

    /// Joins two lists together.
    pub fn join(&mut self, other: IntrusiveList<T>) {
        other.release(|element| self.push(element));
    }
}

impl<T> Drop for IntrusiveList<T> {
    fn drop(&mut self) {
        let mut element = self.head;
        while !element.is_null() {
            let element_ref = unsafe { OwnedMutPtr::new_from_raw(element) };
            let next = element_ref.next;

            drop(element_ref);

            element = next;
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct IntrusiveItem<T> {
    inner: T,
    next: *mut IntrusiveItem<T>,
    prev: *mut IntrusiveItem<T>,
}

impl<T> IntrusiveItem<T> {
    pub const fn new(inner: T) -> Self {
        Self {
            inner,
            next: core::ptr::null_mut(),
            prev: core::ptr::null_mut(),
        }
    }
}

impl<T> core::ops::Deref for IntrusiveItem<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> core::ops::DerefMut for IntrusiveItem<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[derive(Debug)]
pub struct IntrusiveListIter<'a, T> {
    head_item: Option<&'a IntrusiveItem<T>>,
    tail_item: *const IntrusiveItem<T>,
}

impl<'a, T> Iterator for IntrusiveListIter<'a, T> {
    type Item = &'a IntrusiveItem<T>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.head_item.take() {
            Some(item) => {
                if item as *const _ == self.tail_item {
                    // This is the last item, we set both iters to None
                    self.head_item = None;
                    self.tail_item = core::ptr::null_mut();
                } else if !item.next.is_null() {
                    self.head_item = Some(unsafe { &*item.next });
                }
                Some(item)
            }
            None => None,
        }
    }
}

impl<'a, T> DoubleEndedIterator for IntrusiveListIter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.tail_item.is_null() {
            None
        } else {
            let element = unsafe { &*self.tail_item };
            if self.tail_item == self.head_item.unwrap() as *const _ {
                // This is the last item, we set both iters to None
                self.head_item = None;
                self.tail_item = core::ptr::null_mut();
            } else {
                self.tail_item = element.prev;
            }
            Some(element)
        }
    }
}

#[derive(Debug)]
pub struct IntrusiveListMutIter<'a, T> {
    head_item: Option<&'a mut IntrusiveItem<T>>,
    tail_item: *mut IntrusiveItem<T>,
}

impl<'a, T> Iterator for IntrusiveListMutIter<'a, T> {
    type Item = &'a mut IntrusiveItem<T>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.head_item.take() {
            Some(item) => {
                if item as *const _ == self.tail_item {
                    // This is the last item, we set both iters to None
                    self.head_item = None;
                    self.tail_item = core::ptr::null_mut();
                } else if !item.next.is_null() {
                    self.head_item = Some(unsafe { &mut *item.next });
                }
                Some(item)
            }
            None => None,
        }
    }
}

impl<'a, T> DoubleEndedIterator for IntrusiveListMutIter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.tail_item.is_null() {
            None
        } else {
            let element = unsafe { &mut *self.tail_item };
            if self.tail_item == (*self.head_item.as_mut().unwrap()) as *mut _ {
                // This is the last item, we set both iters to None
                self.head_item = None;
                self.tail_item = core::ptr::null_mut();
            } else {
                self.tail_item = element.prev;
            }
            Some(element)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use core::ops::Deref;

    #[test]
    fn can_append_to_list() {
        let a = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(32)));
        let b = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(23)));
        let c = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(84)));

        let mut list = IntrusiveList::<_>::new();
        list.push(a);
        list.push(b);
        list.push(c);

        let vector: Vec<_> = list.iter().map(|item| item.inner).collect();
        assert_eq!(vector, vec![32, 23, 84]);

        let d = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(843)));
        list.push(d);

        let vector: Vec<_> = list.iter().map(|item| item.inner).collect();
        assert_eq!(vector, vec![32, 23, 84, 843]);

        list.release(|element| {
            let _ = unsafe { element.into_box() };
        });
    }

    #[test]
    fn rev_iter() {
        let a = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(32)));
        let b = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(23)));
        let c = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(84)));

        let mut list = IntrusiveList::<_>::new();
        list.push(a);
        list.push(b);
        list.push(c);

        let vector: Vec<_> = list.iter().rev().map(|item| item.inner).collect();
        assert_eq!(vector, vec![84, 23, 32]);

        let d = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(843)));
        list.push(d);

        let vector: Vec<_> = list.iter().rev().map(|item| item.inner).collect();
        assert_eq!(vector, vec![843, 84, 23, 32]);

        list.release(|element| {
            let _ = unsafe { element.into_box() };
        });
    }

    #[test]
    fn double_ended_iter() {
        let a = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(32)));
        let b = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(23)));
        let c = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(84)));

        let mut list = IntrusiveList::<_>::new();
        list.push(a);
        list.push(b);
        list.push(c);

        let d = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(843)));
        list.push(d);

        let mut iter = list.iter().map(|item| item.inner);
        assert_eq!(iter.next(), Some(32));
        assert_eq!(iter.next_back(), Some(843));
        assert_eq!(iter.next(), Some(23));
        assert_eq!(iter.next_back(), Some(84));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next_back(), None);

        list.release(|element| {
            let _ = unsafe { element.into_box() };
        });
    }

    #[test]
    fn pop_entry() {
        let a = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(32)));
        let b = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(23)));
        let c = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(84)));

        let mut list = IntrusiveList::<_>::new();
        list.push(a);
        list.push(b);
        list.push(c);

        let vector: Vec<_> = list.iter().map(|item| item.inner).collect();
        assert_eq!(vector, vec![32, 23, 84]);

        let removed_element = list.pop().expect("There is no element to pop");
        assert_eq!(*removed_element.deref().deref(), 32);

        let _ = unsafe { removed_element.into_box() };

        let vector: Vec<_> = list.iter().map(|item| item.inner).collect();
        assert_eq!(vector, vec![23, 84]);

        list.release(|element| {
            let _ = unsafe { element.into_box() };
        });
    }

    #[test]
    fn remove_by_index() {
        let a = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(32)));
        let b = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(23)));
        let c = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(84)));

        let mut list = IntrusiveList::<_>::new();
        list.push(a);
        list.push(b);
        list.push(c);

        let vector: Vec<_> = list.iter().map(|item| item.inner).collect();
        assert_eq!(vector, vec![32, 23, 84]);

        let removed_element = list.remove(1).expect("Could not remove element");
        assert_eq!(*removed_element.deref().deref(), 23);

        let _ = unsafe { removed_element.into_box() };

        let vector: Vec<_> = list.iter().map(|item| item.inner).collect();
        assert_eq!(vector, vec![32, 84]);

        list.release(|element| {
            let _ = unsafe { element.into_box() };
        });
    }

    #[test]
    fn remove_by_predicate() {
        let a = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(32)));
        let b = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(23)));
        let c = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(84)));
        let d = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(234)));
        let e = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(84)));

        let mut list = IntrusiveList::<_>::new();
        list.push(a);
        list.push(b);
        list.push(c);
        list.push(d);
        list.push(e);

        let vector: Vec<_> = list.iter().map(|item| item.inner).collect();
        assert_eq!(vector, vec![32, 23, 84, 234, 84]);

        let removed_list = list.drain_filter(|element| *element.deref() == 84);

        let vector: Vec<_> = list.iter().map(|item| item.inner).collect();
        assert_eq!(vector, vec![32, 23, 234]);

        let vector: Vec<_> = removed_list.iter().map(|item| item.inner).collect();
        assert_eq!(vector, vec![84, 84]);

        removed_list.release(|element| {
            let _ = unsafe { element.into_box() };
        });

        list.release(|element| {
            let _ = unsafe { element.into_box() };
        });
    }

    #[test]
    fn join_two_lists() {
        let a = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(32)));
        let b = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(23)));
        let c = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(84)));
        let d = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(234)));
        let e = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(84)));

        let mut list = IntrusiveList::<_>::new();
        list.push(a);
        list.push(b);
        list.push(c);
        list.push(d);
        list.push(e);

        let vector: Vec<_> = list.iter().map(|item| item.inner).collect();
        assert_eq!(vector, vec![32, 23, 84, 234, 84]);

        let removed_list = list.drain_filter(|element| *element.deref() == 84);

        let vector: Vec<_> = list.iter().map(|item| item.inner).collect();
        assert_eq!(vector, vec![32, 23, 234]);

        let vector: Vec<_> = removed_list.iter().map(|item| item.inner).collect();
        assert_eq!(vector, vec![84, 84]);

        list.join(removed_list);

        let vector: Vec<_> = list.iter().map(|item| item.inner).collect();
        assert_eq!(vector, vec![32, 23, 234, 84, 84]);

        list.release(|element| {
            let _ = unsafe { element.into_box() };
        });
    }

    #[test]
    fn double_ended_mut_iter() {
        let a = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(32)));
        let b = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(23)));
        let c = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(84)));

        let mut list = IntrusiveList::<_>::new();
        list.push(a);
        list.push(b);
        list.push(c);

        let d = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(843)));
        list.push(d);

        list.iter_mut().for_each(|element| {
            if **element == 84 {
                **element = 123;
            }
        });

        let mut iter = list.iter().map(|item| item.inner);
        assert_eq!(iter.next(), Some(32));
        assert_eq!(iter.next_back(), Some(843));
        assert_eq!(iter.next(), Some(23));
        assert_eq!(iter.next_back(), Some(123));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next_back(), None);

        let mut iter = list.iter_mut().map(|item| item.inner);
        assert_eq!(iter.next(), Some(32));
        assert_eq!(iter.next_back(), Some(843));
        assert_eq!(iter.next(), Some(23));
        assert_eq!(iter.next_back(), Some(123));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next_back(), None);

        list.release(|element| {
            let _ = unsafe { element.into_box() };
        });
    }

    #[test]
    fn empty_list() {
        let mut list = IntrusiveList::<u32>::new();
        assert!(list.pop().is_none());
        assert!(list.iter().next().is_none());
        assert!(list.iter_mut().next().is_none());
        assert!(list.remove(0).is_none());
        assert!(list.remove(2).is_none());
    }

    #[test]
    fn length() {
        let mut list = IntrusiveList::<_>::new();
        assert_eq!(list.len(), 0);
        list.push(OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(32))));
        assert_eq!(list.len(), 1);
    }
}
