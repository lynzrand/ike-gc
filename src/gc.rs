use core::panic;
use std::{cell::Cell, collections::VecDeque, ptr::NonNull};

use log::{debug, error, warn};
use memmap2::MmapMut;
use slotmap::{new_key_type, SlotMap};

use crate::{gc_ptr::Gc, GCHeader, VTPtr, VTable};

fn header_from_ptr<T>(ptr: *const T) -> *mut GCHeader {
    let ptr = ptr as *const GCHeader as *mut GCHeader;
    unsafe { ptr.sub(1) }
}

fn ptr_from_header<T>(header: *const GCHeader) -> *const T {
    unsafe { header.add(1) as *const T }
}

new_key_type! {
    pub struct HandleKey;
}

pub struct Handle<T> {
    key: HandleKey,
    _marker: std::marker::PhantomData<T>,
}

pub struct GCAlloc {
    _mmap: MmapMut,

    from_half: *mut u8,
    to_half: *mut u8,
    space_size: usize,

    from_cursor: usize,

    in_gc: bool,

    work_list: VecDeque<*const GCHeader>,

    handles: SlotMap<HandleKey, NonNull<u8>>,

    gc_count: usize,
    meta_total_allocated: usize,
    meta_high_water_mark: usize,
}

#[derive(Debug, Default)]
pub struct GCMeta {
    pub currently_allocated: usize,
    pub gc_count: usize,
    pub total_allocated: usize,
    pub high_water_mark: usize,
}

const ALIGNMENT: usize = 16;

impl GCAlloc {
    pub fn new(sz: usize) -> Self {
        // Request 2*sz bytes from the system, and split it into two halves.
        let mmap = MmapMut::map_anon(2 * sz).unwrap();
        let ptr = mmap.as_ptr();
        let from_half = ptr as *mut u8;
        let to_half = unsafe { ptr.add(sz) } as *mut u8;

        GCAlloc {
            _mmap: mmap,
            from_half,
            to_half,
            from_cursor: 0,
            space_size: sz,
            in_gc: false,
            work_list: VecDeque::new(),
            handles: SlotMap::with_key(),

            gc_count: 0,
            meta_total_allocated: 0,
            meta_high_water_mark: 0,
        }
    }

    pub fn metadata(&self) -> GCMeta {
        GCMeta {
            currently_allocated: self.from_cursor,
            gc_count: self.gc_count,
            total_allocated: self.meta_total_allocated,
            high_water_mark: self.meta_high_water_mark,
        }
    }

    /// Acquire a handle to a pointer of type T. The pointer must be allocated
    /// by [`GCAlloc::allocate`].
    pub fn acquire_handle<T>(&mut self, ptr: Gc<T>) -> Handle<T> {
        let ptr = ptr.get();
        assert!(ptr as usize % ALIGNMENT == 0);
        assert!(ptr as usize >= self.from_half as usize);
        let key = self.handles.insert(NonNull::new(ptr as *mut u8).unwrap());
        Handle {
            key,
            _marker: std::marker::PhantomData,
        }
    }

    /// Get a handle to a pointer of type T.
    pub fn get_handle<T>(&self, handle: &Handle<T>) -> Gc<T> {
        Gc::new(self.handles[handle.key].as_ptr() as *const T)
    }

    /// Release a handle.
    pub fn release_handle<T>(&mut self, handle: Handle<T>) {
        self.handles.remove(handle.key);
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn allocate_typed<T: Sized>(&mut self, vt: *const VTable, v: T) -> Option<Gc<T>> {
        unsafe {
            let init_gc_cnt = self.gc_count;
            let ptr = self.allocate(vt, std::mem::size_of::<T>())?;
            let ptr = ptr.cast();
            (ptr.get() as *mut T).write(v);
            // Might have gc during allocation, so we need to run the rewrite callback
            if self.gc_count != init_gc_cnt {
                ((*vt).rewrite_cb)(self, ptr.get() as *const u8);
            }
            Some(ptr)
        }
    }

    pub fn allocate(&mut self, vt: *const VTable, raw_sz: usize) -> Option<Gc<u8>> {
        if self.in_gc {
            error!("Allocation during GC");
            return None;
        }

        let sz = (std::mem::size_of::<GCHeader>() + raw_sz).next_multiple_of(ALIGNMENT);
        let available = self.space_size - self.from_cursor;
        if sz > available {
            debug!("Allocate size {} exceeds available space {}", sz, available);
            self.collect();

            let available = self.space_size - self.from_cursor;
            if sz > available {
                warn!("Out of memory: No space for allocation even after GC");
                return None;
            }
        }

        let start_ptr = unsafe { self.from_half.add(self.from_cursor) };
        let header = GCHeader {
            vt: Cell::new(VTPtr::new(vt)),
            sz,
        };
        debug!("Allocating {} + header bytes at {:?}", sz, start_ptr);

        unsafe {
            std::ptr::write(start_ptr as *mut GCHeader, header);
        }

        self.from_cursor += sz;
        self.meta_total_allocated += sz;
        self.meta_high_water_mark = self.meta_high_water_mark.max(self.from_cursor);
        let ptr = unsafe { start_ptr.add(std::mem::size_of::<GCHeader>()) };

        // Write a free block after the allocated block
        let free_header = GCHeader {
            vt: Cell::new(VTPtr::new_free()),
            sz: available - sz,
        };
        let free_ptr = unsafe { start_ptr.add(sz) as *mut GCHeader };
        debug!(
            "Writing free block of size {} at {:?}",
            available - sz,
            free_ptr
        );
        unsafe {
            std::ptr::write(free_ptr, free_header);
        }

        Some(Gc::new(ptr))
    }

    pub fn collect(&mut self) {
        if self.in_gc {
            panic!("Recursive GC");
        }

        debug!("Starting GC");

        self.in_gc = true;
        self.gc_count += 1;

        // Mark phase
        // Gather root set
        for handle in self.handles.values() {
            debug!("Adding handle {:p} to work list", handle.as_ptr());
            self.work_list.push_back(header_from_ptr(handle.as_ptr()));
        }

        // Process work list
        while let Some(ptr) = self.work_list.pop_front() {
            let hdr = unsafe { ptr.as_ref().unwrap() };

            if hdr.mark() {
                continue;
            }
            debug!("Marking {:p}", ptr);

            // Call the mark callback
            let vt = hdr.vt.get();
            if vt.is_free() {
                panic!("Free block in work list");
            }
            let vt = vt.0.ptr();
            unsafe {
                ((*vt).mark_cb)(self, ptr_from_header(ptr));
            }
        }

        // Copy phase
        let mut to_cursor = 0;
        let mut from_cursor = 0;
        debug!("Copying objects");
        while from_cursor < self.space_size {
            let from_ptr = unsafe { self.from_half.add(from_cursor) };
            let hdr = unsafe { (from_ptr as *const GCHeader).as_ref().unwrap() };
            let sz = hdr.sz;
            assert!(
                sz >= std::mem::size_of::<GCHeader>(),
                "Invalid size smaller than header: {}, found at {:p}",
                sz,
                from_ptr
            );

            if hdr.vt.get().is_free() {
                debug!("Skipping free block {:p}, size {}", from_ptr, sz);
                from_cursor += sz;
                continue;
            }

            let marked = hdr.vt.get().is_marked();
            if !marked {
                debug!("Freeing {:p} as it's not marked", from_ptr);
                unsafe {
                    ((*hdr.vt.get().0.ptr()).free_cb)(self, from_ptr);
                }
                from_cursor += sz;
                continue;
            }

            let to_ptr = unsafe { self.to_half.add(to_cursor) };
            debug!("Copying {:p} to {:p}", from_ptr, to_ptr);
            unsafe {
                std::ptr::copy_nonoverlapping(from_ptr, to_ptr, sz);
            }
            hdr.set_fwd_ptr(ptr_from_header(to_ptr as *const GCHeader));
            let to_hdr = unsafe { (to_ptr as *const GCHeader).as_ref().unwrap() };
            to_hdr.unmark();

            from_cursor += sz;
            to_cursor += sz;
        }
        // Write free block at the end
        let free_header = GCHeader {
            vt: Cell::new(VTPtr::new_free()),
            sz: self.space_size - to_cursor,
        };
        let free_ptr = unsafe { self.to_half.add(to_cursor) as *mut GCHeader };
        debug!(
            "Writing free block of size {} at {:?}",
            self.space_size - to_cursor,
            free_ptr
        );
        unsafe {
            std::ptr::write(free_ptr, free_header);
        }

        let new_from_cursor = to_cursor;

        // Rewrite pointers
        debug!("Rewriting pointers");
        let mut cursor = 0;
        while cursor < new_from_cursor {
            let hdr = unsafe {
                (self.to_half.add(cursor) as *const GCHeader)
                    .as_ref()
                    .unwrap()
            };
            let sz = hdr.sz;
            let total_sz = sz + std::mem::size_of::<GCHeader>();

            if hdr.vt.get().is_free() {
                cursor += total_sz;
                continue;
            }

            unsafe {
                ((*hdr.vt.get().0.ptr()).rewrite_cb)(self, self.to_half.add(cursor));
            }

            cursor += total_sz;
        }
        // rewrite handles
        for handle in self.handles.values_mut() {
            let ptr = handle.as_ptr();
            let header = header_from_ptr(ptr);
            let fwd_ptr = unsafe { &*header }.fwd_ptr();
            debug!("Rewriting handle {:p} to {:p}", ptr, fwd_ptr);
            *handle = NonNull::new(fwd_ptr as *mut u8).unwrap();
        }

        // Swap spaces
        debug!("Swapping spaces");
        std::mem::swap(&mut self.from_half, &mut self.to_half);
        self.from_cursor = new_from_cursor;
        self.in_gc = false;
        debug!("GC done");
    }

    /// Call this to mark a pointer as accessible.
    pub fn mark_accessible<T>(&mut self, ptr: Gc<T>) {
        self.work_list.push_back(header_from_ptr(ptr.get()));
    }

    /// Call this to rewrite a pointer.
    pub fn rewrite_ptr<T>(&mut self, ptr: &Gc<T>) {
        let header = header_from_ptr(ptr);
        let fwd = (unsafe { &*header }).fwd_ptr();
        debug!("Rewriting {:p} to {:p}", ptr.get(), fwd);
        ptr.set(fwd as *const T);
    }
}
