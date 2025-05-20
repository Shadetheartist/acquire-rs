use std::fmt::{Display, Formatter};
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

impl Display for Chain {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
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

    pub fn from_initial(initial: &str) -> Option<Self> {
        match initial {
            "T" => Some(Chain::Tower),
            "L" => Some(Chain::Luxor),
            "A" => Some(Chain::American),
            "W" => Some(Chain::Worldwide),
            "F" => Some(Chain::Festival),
            "C" => Some(Chain::Continental),
            "I" => Some(Chain::Imperial),
            _ => None,
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
        Self([initial_value; NUM_CHAINS as usize])
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
        Self([T::default(); NUM_CHAINS as usize])
    }
}

#[cfg(test)]
mod test {
    
    
    
    
    

    #[test]
    fn test_chain_table() {

    }
}