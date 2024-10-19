use std::cell::Cell;
use std::num::NonZeroUsize;

use tag_ptr::TaggedPtr;

pub mod gc;
pub mod gc_ptr;
mod tag_ptr;
mod vtable;

pub use gc::GCAlloc;
pub use gc::Handle;
pub use vtable::SizeKind;
pub use vtable::VTable;

/// The pointer part of the GC header.
///
/// During GC, after copying, it might be a forward pointer.
/// As the two usage do not overlap, we can use the same field for both.
#[derive(Clone, Copy)]
union VTablePtrUnion {
    /// Used as a pointer to the vtable.
    vt: vtable::VTPtr,
    /// Used as a forward pointer during GC.
    fwd: *const u8,
}

impl From<vtable::VTPtr> for VTablePtrUnion {
    fn from(vt: vtable::VTPtr) -> Self {
        Self { vt }
    }
}

/// A GC object header that's exactly 2 pointers wide.
#[repr(C)]
struct GCHeader {
    /// Table to the vtable and mark bit.
    ///
    /// This field will be occupied as a forward pointer during GC. As this is rarely used,
    /// the usual vtable pointer operations are marked **without** the `unsafe` keyword and assumed
    /// as the default operation. Take care when using this field during GC.
    vt: Cell<VTablePtrUnion>,
    /// The total size of the cell, including the header.
    sz: usize,
}

impl GCHeader {
    /// Get the vtable pointer from the header.
    pub fn get_vt(&self) -> vtable::VTPtr {
        unsafe { self.vt.get().vt }
    }

    /// Mark the object as accessible. Returns true if the object was already marked.
    pub fn mark(&self) -> bool {
        let mut vt = unsafe { self.vt.get().vt };
        let was_marked = vt.is_marked();
        if was_marked {
            return true;
        }
        vt.mark();
        self.vt.set(vt.into());
        false
    }

    pub fn unmark(&self) {
        let mut vt = unsafe { self.vt.get().vt };
        vt.unmark();
        self.vt.set(vt.into());
    }

    /// Write a new forward pointer to the header.
    ///
    /// # Safety
    ///
    /// Only valid during GC.
    pub unsafe fn set_fwd_ptr(&self, ptr: *const u8) {
        let mut vt = self.vt.get();
        vt.fwd = ptr;
        self.vt.set(vt);
    }

    /// Get the forward pointer from the header.
    ///
    /// # Safety
    ///
    /// Only valid during GC.
    pub unsafe fn fwd_ptr(&self) -> *const u8 {
        unsafe { self.vt.get().fwd }
    }
}
