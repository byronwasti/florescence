pub struct StableVec<T> {
    inner: Vec<Option<T>>,
}

impl<T> StableVec<T> {
    pub fn new() -> Self {
        Self { inner: vec![] }
    }

    pub fn push(&mut self, val: T) -> usize {
        if let Some(idx) = self.inner.iter().position(Option::is_none) {
            self.inner[idx] = Some(val);
            idx
        } else {
            self.inner.push(Some(val));
            self.inner.len() - 1
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.inner.get(index).map(|x| x.as_ref()).flatten()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.inner.iter().filter_map(|x| x.as_ref())
    }
}
