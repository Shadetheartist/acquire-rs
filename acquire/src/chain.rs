use std::ops::Index;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Chain {
    Tower,
    Luxor,
    American,
    Worldwide,
    Festival,
    Continental,
    Imperial,
}

const NUM_CHAINS: u8 = 7;
pub const CHAIN_ARRAY: [Chain; NUM_CHAINS as usize] = [
    Chain::Tower,
    Chain::Luxor,
    Chain::American,
    Chain::Worldwide,
    Chain::Festival,
    Chain::Continental,
    Chain::Imperial,
];

impl Chain {
    pub fn initial(&self) -> char {
        match self {
            Chain::Tower => 'T',
            Chain::Luxor => 'L',
            Chain::American => 'A',
            Chain::Worldwide => 'W',
            Chain::Festival => 'F',
            Chain::Continental => 'C',
            Chain::Imperial => 'I',
        }
    }

    pub fn as_index(&self) -> usize {
        *self as usize
    }

    pub fn from_index(idx: usize) -> Chain {
        CHAIN_ARRAY[idx]
    }
}

#[derive(Clone)]
pub struct ChainTable<T: Copy>(pub [T; NUM_CHAINS as usize]);

impl<T: Copy> Index<&Chain> for ChainTable<T> {
    type Output = T;

    fn index(&self, chain_idx: &Chain) -> &Self::Output {
        &self.0[chain_idx.as_index()]
    }
}

impl<T: Copy> ChainTable<T> {

    pub fn new(initial_value: T) -> Self {
        Self {
            0: [initial_value; NUM_CHAINS as usize]
        }
    }

    pub fn set(&mut self, chain: &Chain, value: T) {
        self.0[chain.as_index()] = value;
    }

    pub fn get(&self, chain: &Chain) -> T {
        self.0[chain.as_index()]
    }
}

impl<T: Copy + Default> Default for ChainTable<T> {
    fn default() -> Self {
        Self { 0: [T::default(); NUM_CHAINS as usize] }
    }
}

#[cfg(test)]
mod test {
    use rand::SeedableRng;
    use rand::seq::SliceRandom;
    use crate::{Acquire, Options, Phase, PlayerId, tile};
    use crate::chain::Chain;
    use crate::grid::Slot;

    #[test]
    fn test_chain_table() {

    }
}