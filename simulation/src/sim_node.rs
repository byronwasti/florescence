use crate::{history::*, mail::*, traits::*};
use rand::{Rng, SeedableRng, rngs::StdRng, seq::SliceRandom};
use std::{any::Any, cmp::Ordering, collections::BinaryHeap, panic};
use thiserror::Error;

pub struct SimNode<S: Simulee> {
    mailbox: BinaryHeap<Mail<S::Message>>,
    simulee: Option<S>,
}

impl<S> SimNode<S>
where
    S: Simulee + panic::UnwindSafe,
    S::Event: panic::UnwindSafe,
    S::Message: panic::UnwindSafe,
{
    pub fn step<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        wall_time: u64,
    ) -> Result<HistoricalRecord<S::Snapshot, S::HistoricalEvent>, SimNodeError> {
        let event = self
            .select_event(rng, wall_time)
            .ok_or(SimNodeError::NoEvent)?;

        let snapshot = self
            .simulee
            .as_ref()
            .expect("No simulee available.")
            .snapshot();

        let mail = if event.take_mail() {
            self.mailbox.pop()
        } else {
            None
        };

        let message = mail.map(|x| x.msg.clone());

        let mut simulee = self.simulee.take().expect("No simulee available.");

        let (historical_event, simulee) = panic::catch_unwind(|| {
            let res = simulee.step(event, message, wall_time);
            (res, simulee)
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

        Ok(HistoricalRecord {
            node_snapshot: snapshot,
            event: historical_event,
        })
    }

    fn select_event<R: Rng + ?Sized>(&self, rng: &mut R, wall_time: u64) -> Option<S::Event> {
        let events = self
            .simulee
            .as_ref()
            .expect("No simulee available.")
            .events(wall_time, !self.mailbox.is_empty());
        for (event, probability) in events {
            if rng.random_bool(probability) {
                return Some(event);
            }
        }

        None
    }
}

#[derive(Debug, Error)]
enum SimNodeError {
    #[error("No event")]
    NoEvent,

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
