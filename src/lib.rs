use sweeper::Sweeper;
use tag_ptr::TaggedPtr;

pub mod sweeper;
mod tag_ptr;

#[repr(C)]
pub struct VTable {
    /// Callback on mark. The user is expected to call [`Sweeper::mark_accessible`] on all pointers
    /// in the object.
    mark_cb: unsafe fn(*const Sweeper, *const u8),

    /// Callback on rewrite. The user is expected to call [`Sweeper::rewrite_ptr`] on all pointers
    /// in the object, and update them accordingly.
    rewrite_cb: unsafe fn(*const Sweeper, *const u8),
}

/// A tagged pointer to a VTable, with a mark bit. A null pointer is used to represent a free block.
#[repr(transparent)]
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
    vt: VTPtr,
    sz: usize,
}
