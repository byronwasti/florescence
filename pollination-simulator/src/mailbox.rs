use petgraph::graph::NodeIndex;
use rand::{Rng, RngExt};
use std::{cmp::Ordering, collections::VecDeque};

#[derive(Debug, Default)]
pub struct Mailbox<Message> {
    counter: u64,
    inner: VecDeque<Mail<Message>>,
}

impl<Message> Mailbox<Message> {
    pub fn new() -> Mailbox<Message> {
        Self {
            counter: 0,
            inner: VecDeque::new(),
        }
    }

    pub fn push(&mut self, from: NodeIndex, msg: Message) {
        self.inner.push_back(Mail::new(from, msg));
        self.counter += 1;
    }

    pub fn push_mail(&mut self, mail: Mail<Message>) {
        self.inner.push_back(mail);
    }

    /// Returns a tuple of (Mail, Delivery) to fascillitate history.
    pub fn get_delivery(&mut self) -> Option<Delivery<Message>> {
        let mail = self.inner.pop_front()?;
        Some(Delivery {
            delivered: false,
            mail,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &Mail<Message>> {
        self.inner.iter()
    }
}

#[derive(Debug)]
pub struct Delivery<Message> {
    delivered: bool,
    mail: Mail<Message>,
}

impl<Message: Clone> Delivery<Message> {
    // TODO: better name
    pub fn take(&mut self) -> Mail<Message> {
        self.delivered = true;
        self.mail.clone()
    }

    pub(crate) fn take_final(self) -> Mail<Message> {
        self.mail
    }

    pub fn delivered(&self) -> bool {
        self.delivered
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mail<Message> {
    pub from: NodeIndex,
    pub msg: Message,
}

impl<M> Mail<M> {
    pub fn new(from: NodeIndex, msg: M) -> Mail<M> {
        Self { from, msg }
    }
}
