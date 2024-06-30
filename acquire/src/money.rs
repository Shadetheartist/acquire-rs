use ahash::HashMap;
use lazy_static::lazy_static;
use crate::{Acquire, PlayerId};
use crate::chain::Chain;
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
    pub fn chain_bonus(&self, chain: Chain) -> HashMap<PlayerId, u32> {
        let players_with_stock: Vec<&Player> = self.players
            .iter()
            .filter(|player| {
                player.stocks.has_any(chain)
            })
            .collect();

        if players_with_stock.is_empty() {
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
            .filter(|p| p.stocks.amount(chain) != most_stock_held)
            .map(|p| p.stocks.amount(chain))
            .max()
            .unwrap_or(0);

        let players_with_most_stock: Vec<&&Player> = players_with_stock
            .iter()
            .filter(|p| p.stocks.amount(chain) == most_stock_held)
            .collect();

        // not including zero
        let players_with_second_most_stock: Vec<&&Player> = players_with_stock
            .iter()
            .filter(|p| {
                second_most_stock_held != 0 &&
                    p.stocks.amount(chain) == second_most_stock_held
            })
            .collect();


        let chain_size = self.grid.chain_size(chain);
        let chain_value = chain_value(chain, chain_size);
        let total_major_bonus = chain_value * 10;
        let total_minor_bonus = chain_value * 5;

        // share first place rewards combined, second place gets shit all
        if players_with_most_stock.len() > 1 || (players_with_most_stock.len() == 1 && players_with_second_most_stock.is_empty()) {
            let split_bonus = round_up_to_nearest_hundred(total_major_bonus / players_with_most_stock.len() as u32);
            return players_with_most_stock.iter().map(|player| (player.id, split_bonus)).collect();
        } else if players_with_most_stock.len() == 1 && !players_with_second_most_stock.is_empty() {
            let mut map = HashMap::default();

            map.insert(players_with_most_stock[0].id, total_major_bonus);

            let split_minor_bonus = round_up_to_nearest_hundred(total_minor_bonus / players_with_second_most_stock.len() as u32);
            for player in players_with_second_most_stock {
                map.insert(player.id, split_minor_bonus);
            }

            return map;
        } else {
            panic!("weird bonus situation")
        }
    }
}

fn round_up_to_nearest_hundred(num: u32) -> u32 {
    ((num + 99) / 100) * 100
}

#[cfg(test)]
mod test {
    use rand::SeedableRng;
    use crate::{Acquire, Options, tile};
    use crate::chain::Chain;
    use crate::money::round_up_to_nearest_hundred;

    #[test]
    fn test_bonus_calc() {
        let rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
        let mut game = Acquire::new(rng, &Options::default());

        game.grid.place(tile!("A1"));
        game.grid.place(tile!("A2"));
        game.grid.fill_chain(tile!("A1"), Chain::American);

    }

    #[test]
    fn test_nearest_hundred(){
        assert_eq!(round_up_to_nearest_hundred(0), 0);
        assert_eq!(round_up_to_nearest_hundred(50), 100);
        assert_eq!(round_up_to_nearest_hundred(175), 200);
        assert_eq!(round_up_to_nearest_hundred(125), 200);
        assert_eq!(round_up_to_nearest_hundred(700), 700);
    }
}