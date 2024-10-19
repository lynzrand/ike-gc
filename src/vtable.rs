use std::num::NonZeroUsize;

use crate::{tag_ptr::TaggedPtr, GCAlloc};

/// Variant to get the size of an object.
pub enum SizeKind {
    /// The size of the object is fixed.
    Fixed(NonZeroUsize),
    /// The size of the object is variable. The callback should return the size of the object.
    Variable(unsafe fn(*const u8) -> NonZeroUsize),
}

impl SizeKind {
    pub const fn fixed(size: usize) -> Self {
        if size == 0 {
            panic!("Size must be greater than 0");
        }
        Self::Fixed(unsafe { NonZeroUsize::new_unchecked(size) })
    }

    pub const fn of<T>() -> Self {
        Self::fixed(std::mem::size_of::<T>())
    }

    pub fn callback(cb: unsafe fn(*const u8) -> NonZeroUsize) -> Self {
        Self::Variable(cb)
    }
}

#[repr(C)]
pub struct VTable {
    // /// The size of the object.
    // pub size: SizeKind,
    /// Callback on mark. The user is expected to call [`Sweeper::mark_accessible`] on all pointers
    /// in the object. The pointer is guaranteed to be valid and points to a live object of the
    /// expected type.
    pub mark_cb: unsafe fn(&mut GCAlloc, *const u8),

    /// Callback on rewrite. The user is expected to call [`Sweeper::rewrite_ptr`] on all pointers
    /// in the object, and update them accordingly. The pointer is guaranteed to be valid and points
    /// to a live object of the expected type.
    pub rewrite_cb: unsafe fn(&mut GCAlloc, *const u8),

    /// Callback on free. The user is expected to free all resources associated with the object.
    pub free_cb: unsafe fn(&mut GCAlloc, *const u8),
}

/// A tagged pointer to a VTable, with a mark bit. A null pointer is used to represent a free block.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VTPtr(TaggedPtr<1, VTable>);

impl VTPtr {
    pub fn new(ptr: *const VTable) -> Self {
        Self(TaggedPtr::new(ptr, 0))
    }

    pub fn new_free() -> Self {
        Self(TaggedPtr::new(std::ptr::null(), 0))
    }

    pub fn ptr(&self) -> *const VTable {
        self.0.ptr()
    }

    pub fn is_free(&self) -> bool {
        self.0.ptr().is_null()
    }

    pub fn mark(&mut self) {
        self.0.set_tag(1);
    }

    pub fn unmark(&mut self) {
        self.0.set_tag(0);
    }

    pub fn is_marked(&self) -> bool {
        self.0.tag() == 1
    }
}
