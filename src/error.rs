use thiserror::Error;

use solana_program::program_error::ProgramError;

#[derive(Error, Debug, Copy, Clone)]
pub enum CodeError {
    #[error("Program arithmetic overflowed")]
    ArithmeticOverflow,

    #[error("Position is already initialized")]
    PositionIsAlreadyInitialized,

    #[error("Position is not initialized")]
    PositionNotInitialized,
}

impl From<CodeError> for ProgramError {
    fn from(e: CodeError) -> Self {
        ProgramError::Custom(e as u32)
    }
}