use std::marker::PhantomData;

const MAX_ALLOCS: usize = 1 << 9;

pub struct GrArena {
    gens: Vec<&'static mut [u64; MAX_ALLOCS]>,
    unused: Vec<usize>,
}

impl GrArena {
    pub fn new() -> Self {
        GrArena {
            gens: Vec::new(),
            unused: Vec::new(),
        }
    }

    pub fn alloc<'a, T>(&'a mut self, v: T) -> Gr<'a, T> {
        loop {
            match self.unused.pop() {
                Some(gen_idx) => {
                    return Gr {
                        ptr: Box::leak(Box::new(v)) as *mut T,
                        gen_idx: gen_idx,
                        arena: self as *mut GrArena,
                        phantom: PhantomData,
                    };
                }
                None => {
                    self.gens.push(Box::leak(Box::new([1; MAX_ALLOCS])));
                    for i in 0..MAX_ALLOCS {
                        self.unused.push(i + (self.gens.len()-1) * MAX_ALLOCS);
                    }
                }
            }
        }
    }
}

pub struct Gr<'a, T> {
    ptr: *mut T,
    gen_idx: usize,
    arena: *mut GrArena,
    phantom: std::marker::PhantomData<&'a u64>,
}

impl<'a, T> Gr<'a, T> {
    unsafe fn gen(&self) -> *mut u64 {
        (*self.arena).gens[self.gen_idx / MAX_ALLOCS]
            .as_mut_ptr()
            .add(self.gen_idx % MAX_ALLOCS)
    }

    pub fn weak(&self) -> Weak<'a, T> {
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
            Box::from_raw(self.ptr);
            let gen = self.gen();
            *gen += 1;
            (*self.arena).unused.push(self.gen_idx);
        }
    }
}

pub struct Weak<'a, T> {
    ptr: *mut T,
    gen: *const u64,
    alloc_gen: u64,
    phantom: std::marker::PhantomData<&'a u64>,
}

impl<'a, T> Weak<'a, T> {
    pub fn get(&self) -> Option<&T> {
        if unsafe { *self.gen } != self.alloc_gen {
            None
        } else {
            unsafe { Some(std::mem::transmute(self.ptr)) }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::GrArena;

    #[test]
    fn it_works() {
        let mut arena = GrArena::new();
        let r1;
        let r2;
        {
            let s = arena.alloc(String::from("Hello World"));
            r1 = s.weak();
            r2 = s.weak();
        }

        let s = r1.get();
        assert_eq!(s, None);
        let s = r2.get();
        assert_eq!(s, None);
    }

    #[test]
    fn many() {
        let mut arena = GrArena::new();

        let mut allocs = Vec::new();
        for _ in 0..10000 {
            allocs.push(arena.alloc(String::from("Hello World")));
        }
    }
}
