use super::{Pollinator, PollinatorInner};

pub struct IdentityMap<T> {
    t: T,
}

impl<T> IdentityMap<T> {
    pub fn set(&mut self, _value: T) {
        todo!()
    }

    pub fn get(&self) -> &T {
        &self.t
    }

    pub fn fold<B, F>(&self, _init: B, _f: F) -> B
    where
        F: FnMut(B, T) -> B,
    {
        todo!()
    }

    pub fn apply<F>(&self, _f: F) -> T
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
