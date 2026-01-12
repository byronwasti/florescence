use crate::{
    config::Config,
    history::*,
    mailbox::{Delivery, Mail, Mailbox},
    traits::*,
};
use petgraph::graph::NodeIndex;
use rand::{Rng, SeedableRng, rngs::StdRng, seq::SliceRandom};
use std::{any::Any, cmp::Ordering, panic};
use thiserror::Error;

pub struct SimNode<S: Simulee> {
    mailbox: Mailbox<S::Message>,
    simulee: Option<S>,
}

impl<S: Simulee> SimNode<S> {
    pub fn new<R: Rng + ?Sized>(
        rng: &mut R,
        config: &Config<S::Config>,
        index: usize,
    ) -> SimNode<S> {
        Self {
            mailbox: Mailbox::new(),
            simulee: Some(S::new(rng, config, NodeIndex::new(index))),
        }
    }

    pub fn step<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        wall_time: u64,
        config: &Config<S::Config>,
    ) -> Result<HistoricalRecord<S>, SimNodeError> {
        // Before doing anything, we want to snapshot the node as-is.
        let snapshot = self
            .simulee
            .as_ref()
            .expect("No simulee available.")
            .clone();

        let mut delivery = self.mailbox.get_delivery();
        let mut simulee = self.simulee.take().expect("No simulee available.");
        let config = config.clone();
        let seed = rng.random();
        let (event, msgs_out, simulee, delivery) = panic::catch_unwind(move || {
            // Can't pass an Rng across the unwind boundary, so just reseed a new one.
            let mut rng = StdRng::seed_from_u64(seed);
            let (event, msgs_out) = simulee.step(&mut rng, &config, wall_time, &mut delivery);
            (event, msgs_out, simulee, delivery)
        })
        .map_err(|err| {
            if let Some(err) = err.downcast_ref::<&str>() {
                SimNodeError::Panic(err.to_string())
            } else if let Some(err) = err.downcast_ref::<String>() {
                SimNodeError::Panic(err.to_owned())
            } else {
                SimNodeError::PanicUnknown
            }
        })?;
        self.simulee = Some(simulee);

        let msg_in = if let Some(delivery) = delivery {
            let delivered = delivery.delivered();
            let mail = delivery.take_final();
            if delivered {
                Some(mail)
            } else {
                self.mailbox.push(mail);
                None
            }
        } else {
            None
        };

        Ok(HistoricalRecord {
            snapshot,
            event,
            msg_in,
            msgs_out,
        })
    }

    /*
    fn select_action<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        wall_time: u64,
        config: &S::Config,
    ) -> Option<S::Action> {
        let actions = self
            .simulee
            .as_ref()
            .expect("No simulee available.")
            .list_actions(wall_time, !self.mailbox.is_empty(), config);
        for (action, probability) in actions {
            if rng.random_bool(probability) {
                return Some(action);
            }
        }

        None
    }
    */
}

#[derive(Debug, Error)]
pub enum SimNodeError {
    #[error("No action")]
    NoAction,

    #[error("Node hit a panic: {0}")]
    Panic(String),

    #[error("Node hit a panic (unknown payload)")]
    PanicUnknown,
}

/*
#[derive(Debug)]
pub struct SimNode {
    mailbox: BinaryHeap<Mail>,
    inner: PollinationNode<NodeIndex>,
    last_heartbeat: u64,
    last_propagation: u64,
    last_reap: u64,
}

impl Default for SimNode {
    fn default() -> SimNode {
        SimNode {
            inner: PollinationNode::new(
                Uuid::from_u128(0),
                Topic::new("Test".to_string()),
                NodeIndex::new(0),
            ),
            ..Default::default()
        }
    }
}

impl SimNode {
    /// Time is only `peace_time`; we don't want to trigger timeouts on normal prop of events
    /// TODO: Allow more propagation timing shenanigans
    pub fn step<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        time: u64,
        config: &StepConfig,
    ) -> Option<HistoricalEvent> {
        if rng.random_bool(1. / (1. + self.mailbox.len() as f64)) {
            if let h @ Some(_) = self.step_timeout(rng, time, config) {
                return h;
            }
        }

        self.step_mailbox(rng, time, config)
    }

    fn step_timeout<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        time: u64,
        config: &StepConfig,
    ) -> Option<HistoricalEvent> {
        println!("Step timeout");

        if time - self.last_reap > config.timeout_reap {
            self.last_reap = time;

            if self.inner.reap_souls() {
                return Some(HistoricalEvent::GrimTheReaper);
            }
        }

        if time - self.last_heartbeat > config.timeout_heartbeat || self.last_heartbeat == 0 {
            self.last_heartbeat = time;

            if let Some(msg) = self.inner.msg_heartbeat() {
                return Some(HistoricalEvent::Heartbeat { msg });
            }

            let msg = self.inner.msg_new_member().unwrap();
            Some(HistoricalEvent::NewMember { msg })
        } else {
            None
        }
    }

    fn step_mailbox<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        time: u64,
        config: &StepConfig,
    ) -> Option<HistoricalEvent> {
        println!("Step mailbox");

        let in_msg = self.mailbox.pop()?.msg;

        let out = self.inner.handle_message(in_msg.clone());
        match out {
            Ok(PollinationResponse { response, .. }) => Some(HistoricalEvent::HandleMessage {
                in_msg,
                out_msg: response,
            }),

            Err(error) => Some(HistoricalEvent::HandleMessageError { msg: in_msg, error }),
        }
    }
}
*/
