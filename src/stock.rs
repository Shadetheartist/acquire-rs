use ahash::HashMap;
use thiserror::Error;
use crate::Chain;

#[derive(Clone, Default)]
pub struct Stocks {
    stocks: HashMap<Chain, u8>,
}

#[derive(Error, Debug)]
pub enum StockError {
    #[error("player does not have the funds to buy this")]
    InsufficientFunds
}

impl From<HashMap<Chain, u8>> for Stocks {
    fn from(value: HashMap<Chain, u8>) -> Self {
        Self { stocks: value }
    }
}

impl Stocks {

    pub fn amount(&self, chain: Chain) -> u8 {
        if self.stocks.contains_key(&chain) {
            self.stocks[&chain]
        } else {
            0
        }
    }

    pub fn has_any(&self, chain: Chain) -> bool {
        self.stocks.contains_key(&chain)
    }

    pub fn has_amount(&self, chain: Chain, amount: u8) -> bool {
        if amount == 0 {
            return true;
        }

        if self.stocks.contains_key(&chain) == false {
            return false;
        }

        self.stocks[&chain] >= amount
    }

    pub fn deposit(&mut self, chain: Chain, amount: u8) {
        if amount == 0 {
            return;
        }

        self.stocks.entry(chain).and_modify(|n| *n += amount).or_insert(amount);
    }

    pub fn withdraw(&mut self, chain: Chain, amount: u8) -> Result<(), StockError> {
        if amount == 0 {
            return Ok(());
        }

        if self.has_amount(chain, amount) == false {
            return Err(StockError::InsufficientFunds);
        }

        self.stocks.entry(chain).and_modify(|n| *n -= amount);

        if self.stocks[&chain] == 0 {
            self.stocks.remove(&chain);
        }

        Ok(())
    }
}