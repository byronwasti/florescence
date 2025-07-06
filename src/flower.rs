use crate::{
    nucleus::{Nucleus, NucleusError},
    engine::{Engine, EngineRequest, EngineEvent},
};

pub struct Flower<E: Engine<PollinationMessage>> {
    nucleus: Nucleus<E::Addr>,
    engine: E,
    seed_list: Vec<E::Addr>,
}

impl<E> Flower<E>
where
    E: Engine<PollinationMessage>,
    E::Addr: Clone + Serialize + for<'de> Deserialize<'de> + Hash + fmt::Display,
{
}
