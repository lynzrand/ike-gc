use std::cell::Cell;

use gc::GCAlloc;
use tag_ptr::TaggedPtr;

pub mod gc;
mod tag_ptr;

#[repr(C)]
pub struct VTable {
    /// Callback on mark. The user is expected to call [`Sweeper::mark_accessible`] on all pointers
    /// in the object. The pointer is guaranteed to be valid and points to a live object of the
    /// expected type.
    mark_cb: unsafe fn(&mut GCAlloc, *const u8),

    /// Callback on rewrite. The user is expected to call [`Sweeper::rewrite_ptr`] on all pointers
    /// in the object, and update them accordingly. The pointer is guaranteed to be valid and points
    /// to a live object of the expected type.
    rewrite_cb: unsafe fn(&mut GCAlloc, *const u8),
}

/// A tagged pointer to a VTable, with a mark bit. A null pointer is used to represent a free block.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct VTPtr(TaggedPtr<1, VTable>);

impl VTPtr {
    fn new(ptr: *const VTable) -> Self {
        Self(TaggedPtr::new(ptr, 0))
    }

    fn new_free() -> Self {
        Self(TaggedPtr::new(std::ptr::null(), 0))
    }

    fn is_free(&self) -> bool {
        self.0.ptr().is_null()
    }

    fn mark(&mut self) {
        self.0.set_tag(1);
    }

    fn unmark(&mut self) {
        self.0.set_tag(0);
    }

    fn is_marked(&self) -> bool {
        self.0.tag() == 1
    }
}

/// A GC object header that's exactly 2 pointers wide.
#[repr(C)]
struct GCHeader {
    vt: Cell<VTPtr>,
    sz: usize,
}

impl GCHeader {
    /// Mark the object as accessible. Returns true if the object was already marked.
    pub fn mark(&self) -> bool {
        let mut vt = self.vt.get();
        let was_marked = vt.is_marked();
        if was_marked {
            return true;
        }
        vt.mark();
        self.vt.set(vt);
        false
    }

    pub fn unmark(&self) {
        let mut vt = self.vt.get();
        vt.unmark();
        self.vt.set(vt);
    }

    /// Write a new forward pointer to the header.
    pub fn set_fwd_ptr(&self, ptr: *const u8) {
        let mut vt = self.vt.get();
        vt.0.set_ptr(ptr as *const VTable); // It's actually not a VTable, but we don't care
        self.vt.set(vt);
    }

    /// Get the forward pointer from the header.
    pub fn fwd_ptr(&self) -> *const u8 {
        self.vt.get().0.ptr() as *const u8 // It's actually not a VTable, but we don't care
    }
}
