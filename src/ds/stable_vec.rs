#[derive(Debug, Clone)]
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

    #[allow(unused)]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.inner.get(index).and_then(|x| x.as_ref())
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.inner.get_mut(index).and_then(|x| x.as_mut())
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.inner.iter().filter_map(|x| x.as_ref())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.inner.iter_mut().filter_map(|x| x.as_mut())
    }

    pub fn enumerate(&self) -> impl Iterator<Item = (usize, &T)> {
        self.inner
            .iter()
            .enumerate()
            .filter_map(|(idx, t)| t.as_ref().map(|x| (idx, x)))
    }

    pub fn remove(&mut self, index: usize) -> Option<T> {
        let val = self.inner.get_mut(index)?;
        val.take()
    }
}
