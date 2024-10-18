use std::fmt::Debug;

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
        (self.ptr & !(1 << (TAG_BITS - 1))) as *const T
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

impl<T> Debug for TaggedPtr<1, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:p} ({})", self.ptr(), self.tag())
    }
}
impl<const TAG_BITS: usize, T> Clone for TaggedPtr<TAG_BITS, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<const TAG_BITS: usize, T> Copy for TaggedPtr<TAG_BITS, T> {}

impl<const TAG_BITS: usize, T> PartialEq for TaggedPtr<TAG_BITS, T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr == other.ptr
    }
}

impl<const TAG_BITS: usize, T> Eq for TaggedPtr<TAG_BITS, T> {}

impl<const TAG_BITS: usize, T> PartialOrd for TaggedPtr<TAG_BITS, T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.ptr.partial_cmp(&other.ptr)
    }
}

impl<const TAG_BITS: usize, T> Ord for TaggedPtr<TAG_BITS, T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ptr.cmp(&other.ptr)
    }
}

impl<const TAG_BITS: usize, T> std::hash::Hash for TaggedPtr<TAG_BITS, T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ptr.hash(state);
    }
}
