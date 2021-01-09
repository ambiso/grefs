use std::marker::PhantomData;

const MAX_ALLOCS: usize = 512;

pub struct GrArena {
    gens: [u64; MAX_ALLOCS],
    unused: Vec<usize>,
}

impl GrArena {
    pub fn new() -> Self {
        let mut unused = Vec::with_capacity(MAX_ALLOCS);
        for i in 0..MAX_ALLOCS {
            unused.push(i);
        }
        GrArena {
            gens: [1; MAX_ALLOCS],
            unused: unused,
        }
    }

    pub fn alloc<'a, T>(&'a mut self, v: T) -> Gr<'a, T> {
        let gen_idx = self.unused.pop().unwrap();
        Gr {
            ptr: Box::leak(Box::new(v)) as *mut T,
            gen: unsafe { self.gens.as_mut_ptr().add(gen_idx) },
            phantom: PhantomData,
        }
    }
}

pub struct Gr<'a, T> {
    ptr: *mut T,
    gen: *mut u64,
    phantom: std::marker::PhantomData<&'a u64>,
}

impl<'a, T> Gr<'a, T> {
    pub fn weak(&self) -> Weak<'a, T> {
        Weak {
            ptr: self.ptr,
            gen: self.gen,
            alloc_gen: unsafe { *self.gen },
            phantom: PhantomData,
        }
    }
}

impl<'a, T> Drop for Gr<'a, T> {
    fn drop(&mut self) {
        unsafe {
            Box::from_raw(self.ptr);
            *self.gen += 1;
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
}
