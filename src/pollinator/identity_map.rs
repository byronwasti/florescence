use super::{Pollinator, PollinatorInner};

pub struct IdentityMap<T> {
    t: T,
}

impl<T> IdentityMap<T> {
    fn set(&mut self, value: T) {
        todo!()
    }

    fn get(&self) -> T {
        todo!()
    }

    fn fold<B, F>(&self, init: B, f: F) -> B
    where
        F: FnMut(B, T) -> B,
    {
        todo!()
    }

    fn apply<F>(&self, f: F) -> T
    where
        F: FnMut(T) -> T,
    {
        todo!()
    }
}

impl<T> PollinatorInner for IdentityMap<T> {}

impl<T> Pollinator for IdentityMap<T> {
    type A = IdentityMap<T>;

    fn from_conn() -> (Self, Self::A) {
        todo!()
    }
}
