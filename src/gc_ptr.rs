use std::{cell::Cell, ptr::NonNull};

#[repr(transparent)]
pub struct Gc<T>(Cell<NonNull<T>>);

impl<T> Gc<T> {
    pub fn new(ptr: *const T) -> Self {
        Self(Cell::new(
            NonNull::new(ptr as *mut T).expect("ptr cannot be null"),
        ))
    }

    pub fn get(&self) -> *const T {
        self.0.get().as_ptr()
    }

    pub fn set(&self, ptr: *const T) {
        self.0
            .set(NonNull::new(ptr as *mut T).expect("ptr cannot be null"));
    }

    /// Cast the pointer to a different type.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the new type is valid for the pointer.
    pub unsafe fn cast<U>(&self) -> Gc<U> {
        Gc::new(self.get() as *const U)
    }
}

impl<T> Clone for Gc<T> {
    fn clone(&self) -> Self {
        Gc::new(self.get())
    }
}
