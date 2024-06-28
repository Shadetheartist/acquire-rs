use ahash::HashMap;
use lazy_static::lazy_static;
use crate::{Acquire, Chain};
use crate::player::Player;

lazy_static! {
    static ref CHAIN_TIER_MAP: HashMap<Chain, u8> = {
        let mut m = HashMap::default();
        m.insert(Chain::Tower, 0);
        m.insert(Chain::Luxor, 0);
        m.insert(Chain::American, 1);
        m.insert(Chain::Worldwide, 1);
        m.insert(Chain::Festival, 1);
        m.insert(Chain::Continental, 2);
        m.insert(Chain::Imperial, 2);

        m
    };
}

pub fn chain_value(chain: Chain, size: u16) -> u32 {
    let tier = CHAIN_TIER_MAP[&chain];
    chain_size_value(size) + tier as u32 * 100
}

fn chain_size_value(chain_size: u16) -> u32 {
    match chain_size {
        ..=1 => 0,
        2..=5 => chain_size as u32 * 100,
        6..=10 => 600,
        11..=20 => 700,
        21..=30 => 800,
        31..=40 => 900,
        41.. => 1000,
    }
}

impl Acquire {
    fn chain_bonus(&self, chain: Chain) -> HashMap<Player, u32> {
        let players_with_stock: Vec<&Player> = self.players
            .iter()
            .filter(|player| {
                player.stocks.has_any(chain)
            })
            .collect();

        if players_with_stock.len() == 0 {
            return HashMap::default();
        }

        let most_stock_held = players_with_stock
            .iter()
            .map(|p| p.stocks.amount(chain))
            .max()
            .unwrap();

        if most_stock_held == 0 {
            return HashMap::default();
        }

        let second_most_stock_held = players_with_stock
            .iter()
            .filter(|p| p.stocks.amount(chain) == most_stock_held)
            .map(|p| p.stocks.amount(chain))
            .max()
            .unwrap();

        let players_with_most_stock: Vec<&&Player> = players_with_stock
            .iter()
            .filter(|p| p.stocks.amount(chain) == most_stock_held)
            .collect();

        let players_with_second_most_stock: Vec<&&Player> = players_with_stock
            .iter()
            .filter(|p| {
                second_most_stock_held != 0 &&
                    p.stocks.amount(chain) == second_most_stock_held
            })
            .collect();

        let mut player_payouts = HashMap::default();


        player_payouts
    }
}

#[cfg(test)]
mod test {
    use rand::SeedableRng;
    use crate::{Acquire, Chain, Options, tile};

    #[test]
    fn test_bonus_calc() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
        let mut game = Acquire::new(rng, &Options::default());

        game.grid.place(tile!("A1"));
        game.grid.place(tile!("A2"));
        game.grid.fill_chain(tile!("A1"), Chain::American);

    }
}