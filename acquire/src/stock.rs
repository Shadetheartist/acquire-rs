use thiserror::Error;
use crate::chain::{Chain, ChainTable};

#[derive(Clone)]
pub struct Stocks {
    stocks: ChainTable<u8>,
}

#[derive(Error, Debug)]
pub enum StockError {
    #[error("there is not enough stock to withdraw")]
    InsufficientStock
}

impl Stocks {

    pub fn new(initial_value: u8) -> Self {
        Self {
            stocks: ChainTable::new(initial_value)
        }
    }

    pub fn amount(&self, chain: Chain) -> u8 {
        self.stocks.get(&chain)
    }

    pub fn has_any(&self, chain: Chain) -> bool {
        self.has_amount(chain, 1)
    }

    pub fn has_amount(&self, chain: Chain, amount: u8) -> bool {
        self.stocks[&chain] >= amount
    }

    pub fn deposit(&mut self, chain: Chain, amount: u8) {
        if amount == 0 {
            return;
        }

        self.stocks.set(&chain, self.stocks.get(&chain) + amount);
    }

    pub fn withdraw(&mut self, chain: Chain, withdraw_amount: u8) -> Result<(), StockError> {

        let amount_available = self.stocks.get(&chain);

        if withdraw_amount > amount_available {
            return Err(StockError::InsufficientStock);
        }

        self.stocks.set(&chain, amount_available - withdraw_amount);

        Ok(())
    }
}