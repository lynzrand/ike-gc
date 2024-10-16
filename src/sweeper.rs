pub struct Sweeper {}

impl Sweeper {
    pub fn mark_accessible(&mut self, _ptr: *const u8) {}

    pub fn rewrite_ptr(&mut self, _ptr: *const u8) -> *const u8 {
        std::ptr::null()
    }
}
