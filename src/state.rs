use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{program_error::ProgramError, program_pack::{IsInitialized, Pack, Sealed}};


pub struct Position {
    pub is_initialized: bool,
    pub spread_margin: u64, // percentage of spread to put limit orders at from market price
}

impl Position {
    pub const SEED: &'static str = "position";
    pub const BASE_TOKEN_SEED: &'static str = "base";
    pub const QUOTE_TOKEN_SEED: &'static str = "quote";
}

impl Sealed for Position {}

impl IsInitialized for Position {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for Position {
    const LEN: usize = 1 + 8;

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, Position::LEN];
        let (
            is_initialized,
            spread_margin,
        ) = array_refs![src, 1, 8];

        Ok(Position {
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            spread_margin: u64::from_le_bytes(*spread_margin),
        })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, Position::LEN];
        let (
            is_initialized,
            spread_margin,
        ) = mut_array_refs![dst, 1, 8];
        match self.is_initialized {
            true => is_initialized[0] = 1,
            false => is_initialized[0] = 0,
        };
        *spread_margin = self.spread_margin.to_le_bytes();
    }
}
