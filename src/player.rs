use crate::{PlayerId};
use crate::stock::Stocks;
use crate::tile::Tile;

#[derive(Clone)]
pub struct Player {
    pub id: PlayerId,
    pub tiles: Vec<Tile>,
    pub stocks: Stocks,
    pub money: u32
}
