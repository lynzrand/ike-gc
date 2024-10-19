use ike_gc::{gc_ptr::Gc, GCAlloc, VTable};
use log::info;

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

#[test]
fn test_main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug"));

    let mut gc = GCAlloc::new(65536);

    info!("Before allocation; {:?}", gc.metadata());
    let alloc1 = gc
        .allocate_typed::<Cons>(&CONS_VTABLE, Cons::new(None, None))
        .expect("Malloc failed");
    let alloc2 = gc
        .allocate_typed::<Cons>(&CONS_VTABLE, Cons::new(Some(alloc1.clone()), None))
        .expect("Malloc failed");
    let alloc3 = gc
        .allocate_typed::<Cons>(&CONS_VTABLE, Cons::new(Some(alloc2.clone()), None))
        .expect("Malloc failed");

    let _alloc4 = gc
        .allocate_typed::<Cons>(&CONS_VTABLE, Cons::new(Some(alloc3.clone()), None))
        .expect("Malloc failed");
    let handle3 = gc.acquire_handle(alloc3);

    info!("After allocation; {:?}", gc.metadata());

    info!("Initiate collection");
    gc.collect();

    info!("After collection; {:?}", gc.metadata());

    // match structure
    let cons3 = gc.get_handle(&handle3);
    let cons3 = unsafe { &*cons3.get() };
    assert!(cons3.car.is_some());
    assert!(cons3.cdr.is_none());
    let cons2 = unsafe { &*cons3.car.as_ref().unwrap().get() };
    assert!(cons2.car.is_some());
    assert!(cons2.cdr.is_none());
    let cons1 = unsafe { &*cons2.car.as_ref().unwrap().get() };
    assert!(cons1.car.is_none());
    assert!(cons1.cdr.is_none());

    gc.release_handle(handle3);
    gc.collect();
    info!("After release; {:?}", gc.metadata());
}
