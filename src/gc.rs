use core::panic;
use std::{cell::Cell, collections::VecDeque, ptr::NonNull};

use memmap2::MmapMut;

use crate::{GCHeader, VTPtr, VTable};

fn header_from_ptr(ptr: *const u8) -> *mut GCHeader {
    let ptr = ptr as *const GCHeader as *mut GCHeader;
    unsafe { ptr.sub(1) }
}

fn ptr_from_header(header: *const GCHeader) -> *const u8 {
    let ptr = header as *const u8;
    unsafe { ptr.add(1) }
}

pub trait RootSetProvider {
    fn roots(&self) -> Box<dyn Iterator<Item = *const u8>>;
}

pub struct GCAlloc {
    _mmap: MmapMut,

    from_half: *mut u8,
    to_half: *mut u8,
    space_size: usize,

    from_cursor: usize,

    in_gc: bool,

    work_list: VecDeque<*const GCHeader>,

    root_set_providers: Vec<Box<dyn RootSetProvider>>,
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
            root_set_providers: Vec::new(),
        }
    }

    pub fn add_root_set_provider(&mut self, provider: Box<dyn RootSetProvider>) {
        self.root_set_providers.push(provider);
    }

    pub fn allocate(&mut self, vt: *const VTable, sz: usize) -> Option<NonNull<u8>> {
        if self.in_gc {
            return None;
        }

        let sz = (std::mem::size_of::<GCHeader>() + sz).next_multiple_of(ALIGNMENT);
        let available = self.space_size - self.from_cursor;
        if sz > available {
            self.collect();

            let available = self.space_size - self.from_cursor;
            if sz > available {
                return None;
            }
        }

        let start_ptr = unsafe { self.from_half.add(self.from_cursor) };
        let header = GCHeader {
            vt: Cell::new(VTPtr::new(vt)),
            sz,
        };

        unsafe {
            std::ptr::write(start_ptr as *mut GCHeader, header);
        }

        self.from_cursor += sz;
        let ptr = unsafe { start_ptr.add(std::mem::size_of::<GCHeader>()) };

        // Write a free block after the allocated block
        let free_header = GCHeader {
            vt: Cell::new(VTPtr::new_free()),
            sz: available - sz - std::mem::size_of::<GCHeader>(),
        };
        let free_ptr = unsafe { ptr.add(sz) as *mut GCHeader };
        unsafe {
            std::ptr::write(free_ptr, free_header);
        }

        Some(unsafe { NonNull::new_unchecked(ptr) })
    }

    pub fn collect(&mut self) {
        if self.in_gc {
            panic!("Recursive GC");
        }

        self.in_gc = true;

        // Mark phase
        // Gather root set
        for provider in &self.root_set_providers {
            for root in provider.roots() {
                let header = header_from_ptr(root);
                self.work_list.push_back(header);
            }
        }

        // Process work list
        while let Some(ptr) = self.work_list.pop_front() {
            let hdr = unsafe { ptr.as_ref().unwrap() };

            if hdr.mark() {
                continue;
            }

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
        while from_cursor < self.space_size {
            let from_ptr = unsafe { self.from_half.add(from_cursor) };
            let hdr = unsafe { (from_ptr as *const GCHeader).as_ref().unwrap() };
            let sz = hdr.sz;
            let total_sz = sz + std::mem::size_of::<GCHeader>();

            if hdr.vt.get().is_free() {
                from_cursor += total_sz;
                continue;
            }

            let marked = hdr.vt.get().is_marked();
            if !marked {
                unsafe {
                    ((*hdr.vt.get().0.ptr()).free_cb)(self, from_ptr);
                }
                from_cursor += total_sz;
                continue;
            }

            hdr.unmark();

            let to_ptr = unsafe { self.to_half.add(to_cursor) };
            unsafe {
                std::ptr::copy_nonoverlapping(from_ptr, to_ptr, total_sz);
            }
            hdr.set_fwd_ptr(ptr_from_header(to_ptr as *const GCHeader));
            to_cursor += total_sz;
        }

        // Swap spaces
        std::mem::swap(&mut self.from_half, &mut self.to_half);
        self.from_cursor = to_cursor;
    }

    /// Call this to mark a pointer as accessible.
    pub fn mark_accessible(&mut self, ptr: *const u8) {
        self.work_list.push_back(header_from_ptr(ptr));
    }

    /// Call this to rewrite a pointer.
    pub fn rewrite_ptr(&mut self, ptr: *const u8) -> *const u8 {
        let header = header_from_ptr(ptr);
        (unsafe { &*header }).fwd_ptr()
    }
}
