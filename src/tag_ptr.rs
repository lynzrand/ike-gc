#[derive(Debug)]
#[repr(transparent)]
pub struct TaggedPtr<const TAG_BITS: usize, T> {
    ptr: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<const TAG_BITS: usize, T> TaggedPtr<TAG_BITS, T> {
    pub fn new(ptr: *const T, tag: usize) -> Self {
        assert!(tag < (1 << TAG_BITS));
        assert!(ptr as usize % (1 << TAG_BITS) == 0);
        Self {
            ptr: ptr as usize | tag,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn ptr(&self) -> *const T {
        (self.ptr & !(1 << TAG_BITS)) as *const T
    }

    pub fn tag(&self) -> usize {
        self.ptr & ((1 << TAG_BITS) - 1)
    }

    pub fn set_tag(&mut self, tag: usize) {
        assert!(tag < (1 << TAG_BITS));
        self.ptr = (self.ptr & !(1 << TAG_BITS)) | tag;
    }

    pub fn set_ptr(&mut self, ptr: *const T) {
        assert!(ptr as usize % (1 << TAG_BITS) == 0);
        self.ptr = (ptr as usize) | self.tag();
    }
}
