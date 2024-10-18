use ike_gc::{gc_ptr::Gc, GCAlloc, VTable};

struct Cons {
    car: Option<Gc<Cons>>,
    cdr: Option<Gc<Cons>>,
}

impl Cons {
    fn new(car: Option<Gc<Cons>>, cdr: Option<Gc<Cons>>) -> Self {
        Self { car, cdr }
    }
}

fn cons_mark(gc: &mut GCAlloc, ptr: *const u8) {
    let cons = unsafe { &*(ptr as *const Cons) };
    if let Some(car) = &cons.car {
        gc.mark_accessible(car.clone());
    }
    if let Some(cdr) = &cons.cdr {
        gc.mark_accessible(cdr.clone());
    }
}

fn cons_free(_gc: &mut GCAlloc, _ptr: *const u8) {
    // noop
}

fn cons_rewrite(gc: &mut GCAlloc, ptr: *const u8) {
    let cons = unsafe { &*(ptr as *const Cons) };
    if let Some(car) = &cons.car {
        gc.rewrite_ptr(car);
    }
    if let Some(cdr) = &cons.cdr {
        gc.rewrite_ptr(cdr);
    }
}

static CONS_VTABLE: VTable = VTable {
    mark_cb: cons_mark,
    rewrite_cb: cons_rewrite,
    free_cb: cons_free,
};

fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug"));

    let mut gc = GCAlloc::new(65536);

    dbg!(gc.metadata());
    let alloc1 = gc
        .allocate_typed::<Cons>(&CONS_VTABLE, Cons::new(None, None))
        .expect("Malloc failed");
    let alloc2 = gc
        .allocate_typed::<Cons>(&CONS_VTABLE, Cons::new(Some(alloc1.clone()), None))
        .expect("Malloc failed");
    let alloc3 = gc
        .allocate_typed::<Cons>(&CONS_VTABLE, Cons::new(Some(alloc2.clone()), None))
        .expect("Malloc failed");

    dbg!(gc.metadata());

    let _alloc4 = gc
        .allocate_typed::<Cons>(&CONS_VTABLE, Cons::new(Some(alloc3.clone()), None))
        .expect("Malloc failed");
    let handle3 = gc.acquire_handle(alloc3);

    gc.collect();

    dbg!(gc.metadata());
}
