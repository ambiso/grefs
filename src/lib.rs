// Generational References 
// Adapted from https://vale.dev/blog/generational-references 

use std::{cell::UnsafeCell, marker::PhantomData, ptr::NonNull};

const MAX_ALLOCS: usize = 1 << 9;

struct GrArenaInternal {
    // Instead of packing the generational numbers with the allocation,
    // we use an extra memory region. This way we can avoid having to
    // use a custom allocator that guarantees that the generational numbers
    // are never used for anything other than generational numbers.
    gens: Vec<Box<[u64; MAX_ALLOCS]>>,
    // Free list
    unused: Vec<usize>,
}

pub struct GrArena {
    inner: UnsafeCell<GrArenaInternal>,
}

impl GrArena {
    pub fn new() -> Self {
        GrArena {
            inner: UnsafeCell::new(GrArenaInternal {
                gens: Vec::new(),
                unused: Vec::new(),
            })
        }
    }

    pub fn alloc<'a, T>(&'a self, v: T) -> Gr<'a, T> {
        // Safety:
        // We don't hand out references to the arena.
        // Additionally, nobody else can own the arena mutably, since we borrowed it.
        let arena = unsafe { &mut *self.inner.get() };
        loop {
            match (*arena).unused.pop() {
                Some(gen_idx) => {
                    // Found an unused slot, return a strong reference to it
                    return Gr {
                        ptr: NonNull::from(Box::leak(Box::new(v))),
                        gen_idx: gen_idx,
                        arena: arena as *mut GrArenaInternal,
                        phantom: PhantomData,
                    };
                }
                None => {
                    // Add more slots if we ran out
                    arena.gens.push(Box::new([1; MAX_ALLOCS]));
                    for i in 0..MAX_ALLOCS {
                        arena.unused.push(i + (arena.gens.len()-1) * MAX_ALLOCS);
                    }
                }
            }
        }
    }
}

pub struct Gr<'a, T> {
    // The contained data
    ptr: NonNull<T>,
    // The index into the generational numbers array
    gen_idx: usize,
    // A pointer to the owning arena.
    // Could be removed if we only had a single global arena.
    arena: *mut GrArenaInternal,
    // Bind the lifetime of the reference to the lifetime of the generational numbers:
    // Must not outlive the arena
    phantom: std::marker::PhantomData<&'a u64>,
}

impl<'a, T> Gr<'a, T> {
    unsafe fn gen(&self) -> *mut u64 {
        (*self.arena).gens[self.gen_idx / MAX_ALLOCS]
            .as_mut_ptr()
            .add(self.gen_idx % MAX_ALLOCS)
    }

    pub fn weak(&self) -> Weak<'a, T> {
        // Get a pointer to the GN
        let gen = unsafe { self.gen() };
        Weak {
            ptr: self.ptr,
            gen: gen,
            alloc_gen: unsafe { *gen },
            phantom: PhantomData,
        }
    }
}

impl<'a, T> Drop for Gr<'a, T> {
    fn drop(&mut self) {
        unsafe {
            Box::from_raw(self.ptr.as_mut());
            let gen = self.gen();
            *gen += 1;
            (*self.arena).unused.push(self.gen_idx);
        }
    }
}

pub struct Weak<'a, T> {
    // The data
    ptr: NonNull<T>,
    // Unfortunately storing the generational numbers (GNs) separately
    // also means we need to store the location of the GN of interest.
    // Furthermore, we need to dereference 2 pointers to get to the data.
    gen: *const u64,
    // The generational number we expect
    alloc_gen: u64,
    // Must not outlive the arena
    phantom: std::marker::PhantomData<&'a u64>,
}

impl<'a, T> Weak<'a, T> {
    pub fn get(&self) -> Option<&T> {
        // Check if the GNs mismatch
        if unsafe { *self.gen } != self.alloc_gen {
            None
        } else {
            unsafe { Some(self.ptr.as_ref()) }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::GrArena;

    #[test]
    fn it_works() {
        let arena = GrArena::new();
        let r1;
        let r2;
        {
            let s = arena.alloc(String::from("Hello World"));
            // Weak references work as long as the single owner exists
            r1 = s.weak();
            assert_eq!(r1.get(), Some(&String::from("Hello World")));
            r2 = s.weak();
            assert_eq!(r2.get(), Some(&String::from("Hello World")));
        }

        // Once the single owner vanished, the weak references no longer function
        let s = r1.get();
        assert_eq!(s, None);
        let s = r2.get();
        assert_eq!(s, None);
    }

    #[test]
    fn many() {
        // Test that we can allocate and use many things
        let arena = GrArena::new();
        let mut allocs = Vec::new();

        for _ in 0..3 {
            for _ in 0..1500 {
                allocs.push(arena.alloc(String::from("Hello World")));
            }

            for i in allocs.iter() {
                i.weak().get().expect("String should be available");
            }

            let wr;
            {
                let r = allocs.pop().unwrap();
                wr = r.weak();
            }
            // Dropping owning ref should invalidate weak ref
            assert_eq!(wr.get(), None);
            let new_s = arena.alloc(String::from("test"));

            assert_eq!(wr.get(), None);
            let wr2 = new_s.weak();
            let s = wr2.get();
            assert_eq!(s, Some(&String::from("test")));

            // Store all weak refs, drop all owning refs, and test that none can be retrieved
            let mut weak_refs = Vec::new();
            
            for or in allocs.iter() {
                weak_refs.push(or.weak());
            }

            allocs.clear();

            for wr in weak_refs.iter() {
                assert_eq!(wr.get(), None);
            }
        }
    }
}
