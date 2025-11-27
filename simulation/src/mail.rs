use petgraph::graph::NodeIndex;
use std::cmp::Ordering;

#[derive(Debug)]
pub struct Mail<Message> {
    pub sort: u64,
    pub from: NodeIndex,
    pub msg: Message,
}

impl<M> PartialEq for Mail<M> {
    fn eq(&self, other: &Self) -> bool {
        self.sort.eq(&other.sort)
    }
}

impl<M> Eq for Mail<M> {}

impl<M> PartialOrd for Mail<M> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.sort.partial_cmp(&other.sort)
    }
}

impl<M> Ord for Mail<M> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.sort.cmp(&other.sort)
    }
}
