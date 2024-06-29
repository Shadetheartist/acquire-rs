use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;
use thiserror::Error;
use crate::grid::Point;


#[derive(Error, Debug)]
pub enum TileParseError {
    #[error("string is the wrong length")]
    WrongLength,
    #[error("string starts with an invalid letter")]
    InvalidLetter,
    #[error("string end with an invalid number")]
    InvalidNumber,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Tile(pub Point);

impl Tile {
    pub fn new(x: i8, y: i8) -> Self {
        Self(Point { x, y })
    }
}

impl TryFrom<&str> for Tile {
    type Error = TileParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.len() < 2 || value.len() > 3 {
            return Err(TileParseError::WrongLength);
        }

        let Ok(y) = map_letter_to_i8(value.chars().nth(0).unwrap()) else {
            return Err(TileParseError::InvalidLetter);
        };

        let Ok(x) = i8::from_str(&value[1..]) else {
            return Err(TileParseError::InvalidNumber);
        };

        Ok(Tile::new(x - 1, y - 1))
    }
}

impl Debug for Tile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_string().as_str())
    }
}

impl Display for Tile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Ok(y) = map_i8_to_letter(self.0.y + 1) {
            f.write_fmt(format_args!("{}{}", y, self.0.x + 1))
        } else {
            f.write_fmt(format_args!("?{}", self.0.x + 1))
        }
    }
}

pub fn map_letter_to_i8(letter: char) -> Result<i8, String> {
    match letter {
        'A'..='Z' => {
            Ok((letter as u8 - b'A') as i8 + 1)
        }
        _ => Err(format!("'{letter}' is not a supported letter (must be uppercase A-Z)"))
    }
}

pub fn map_i8_to_letter(value: i8) -> Result<char, String> {
    match value {
        1..=26 => {
            Ok(char::from_u32('A' as u32 + ((value - 1) as u32)).unwrap())
        }
        _ => Err(format!("'{value}' is not in the correct range"))
    }
}


#[macro_export]
macro_rules! tile {
    ($tile:literal) => {
        $tile.try_into().expect("a valid tile string")
    };
}


#[cfg(test)]
mod test {
    use crate::tile::{map_i8_to_letter, map_letter_to_i8, Tile};

    #[test]
    fn test_map_letter() {
        assert_eq!(map_letter_to_i8('A'), Ok(1));
        assert_eq!(map_letter_to_i8('B'), Ok(2));
        assert_eq!(map_letter_to_i8('C'), Ok(3));
        assert_eq!(map_letter_to_i8('D'), Ok(4));
        assert_eq!(map_letter_to_i8('E'), Ok(5));
        assert_eq!(map_letter_to_i8('F'), Ok(6));
        assert_eq!(map_letter_to_i8('G'), Ok(7));
        assert_eq!(map_letter_to_i8('H'), Ok(8));
        assert_eq!(map_letter_to_i8('I'), Ok(9));
        assert_eq!(map_letter_to_i8('Z'), Ok(26));

        assert_eq!(Ok('A'), map_i8_to_letter(1));
        assert_eq!(Ok('I'), map_i8_to_letter(9));
    }

    #[test]
    fn test_from_str(){
        assert_eq!(Tile::new(0,0), "A1".try_into().unwrap());
        assert_eq!(Tile::new(9,1), "B10".try_into().unwrap());
        assert_eq!(Tile::new(98, 25), "Z99".try_into().unwrap());
    }

    #[test]
    fn test_into_str(){
        let tile: Tile = "A1".try_into().unwrap();
        assert_eq!("A1", tile.to_string().as_str());

        let tile: Tile = "B10".try_into().unwrap();
        assert_eq!("B10", tile.to_string().as_str());

        let tile: Tile = "Z99".try_into().unwrap();
        assert_eq!("Z99", tile.to_string().as_str());
    }
}