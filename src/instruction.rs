use solana_program::program_error::ProgramError;


pub struct CreatePosition {
    pub side: u8, // 1 for bid, 2 for ask
    pub spread_margin: u64, // percentage of spread to put limit orders at from market price
    pub num_base_lots: u64,
    pub client_order_id: u128,
}

pub struct PlaceLimitOrdersWithFreeFunds {
    pub client_order_id: u128,
}

pub enum Instruction {
    /// Creates a position.
    /// Creates position and token accounts. Transfers tokens from user to position and then places a limit order on phoenix.
    /// 
    /// 0. `[]`  Phoenix program.
    /// 1. `[]`  Phoenix log authority.
    /// 2. `[writable]`  Phoenix Market state account.
    /// 3. `[signer, writable]`  Trader account.
    /// 4. `[]`  Position's seat account.
    /// 5. `[writable]`  Position state account. Seeds = [b"position", trader_address, market_address].
    /// 6. `[writable]`  Base token account of position. Seeds = [b"base", position_address, base_mint_address].
    /// 7. `[writable]`  Quote token account of position. Seeds = [b"quote", position_address, quote_mint_address].
    /// 8. `[writable]`  Phoenix Base vault account. Seeds = [b"vault", market_address, base_mint_address] (phoenix program id).
    /// 9. `[writable]`  Phoenix Quote vault account. Seeds = [b"vault", market_address, quote_mint_address] (phoenix program id).
    /// 10. `[]`  Base mint.
    /// 11. `[]`  Quote mint.
    /// 12. `[writable]`  Base token account of trader.
    /// 13. `[writable]`  Quote token account of trader.
    /// 14. `[]`  Token program.
    /// 15. `[]`  System program.
    CreatePosition(CreatePosition),

    /// Cancels a position.
    /// Cancels limit orders on phoenix, withdraws funds, transfers them to user, closes position and token accounts.
    /// 
    /// 0. `[]`  Phoenix program.
    /// 1. `[]`  Phoenix log authority.
    /// 2. `[writable]`  Phoenix Market state account.
    /// 3. `[signer, writable]`  Trader account.
    /// 4. `[writable]`  Position state account. Seeds = [b"position", trader_address, market_address].
    /// 5. `[writable]`  Base token account of position. Seeds = [b"base", position_address, base_mint_address].
    /// 6. `[writable]`  Quote token account of position. Seeds = [b"quote", position_address, quote_mint_address].
    /// 7. `[writable]`  Phoenix Base vault account. Seeds = [b"vault", market_address, base_mint_address] (phoenix program id).
    /// 8. `[writable]`  Phoenix Quote vault account. Seeds = [b"vault", market_address, quote_mint_address] (phoenix program id).
    /// 9. `[writable]`  Base token account of trader.
    /// 10. `[writable]`  Quote token account of trader.
    /// 11. `[]`  Base mint.
    /// 12. `[]`  Quote mint.
    /// 13. `[]`  Token program.
    /// 14. `[]`  System program.
    CancelPosition,

    /// Places new limit orders using free funds.
    /// 
    /// 0. `[]`  Phoenix program.
    /// 1. `[]`  Phoenix log authority.
    /// 2. `[]`  Phoenix Market state account.
    /// 3. `[]`  Trader account.
    /// 4. `[]`  Position's seat account.
    /// 5. `[writable]`  Position state account. Seeds = [b"position", trader_address, market_address].
    /// 6. `[]`  Token program.
    /// 7. `[]`  System program.
    PlaceLimitOrdersWithFreeFunds(PlaceLimitOrdersWithFreeFunds),
}

impl Instruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (tag, rest) = input.split_first().ok_or(ProgramError::InvalidInstructionData)?;

        Ok(match tag {
            0 => {
                let (side, rest) = Self::unpack_u8(rest)?;
                let (spread_margin, rest) = Self::unpack_u64(rest)?;
                let (num_base_lots, rest) = Self::unpack_u64(rest)?;
                let (client_order_id, _rest) = Self::unpack_u128(rest)?;

                Instruction::CreatePosition(CreatePosition {
                    side,
                    spread_margin,
                    num_base_lots,
                    client_order_id,
                })
            }
            1 => Instruction::CancelPosition,
            _ => return Err(ProgramError::InvalidInstructionData),
        })
    }

    fn unpack_u8(input: &[u8]) -> Result<(u8, &[u8]), ProgramError> {
        if input.len() >= 1 {
            let (amount, rest) = input.split_at(1);
            let amount = amount
                .get(..1)
                .and_then(|slice| slice.try_into().ok())
                .map(u8::from_le_bytes)
                .ok_or(ProgramError::InvalidInstructionData)?;
            Ok((amount, rest))
        } else {
            Err(ProgramError::InvalidInstructionData)
        }
    }

    fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
        if input.len() >= 8 {
            let (amount, rest) = input.split_at(8);
            let amount = amount
                .get(..8)
                .and_then(|slice| slice.try_into().ok())
                .map(u64::from_le_bytes)
                .ok_or(ProgramError::InvalidInstructionData)?;
            Ok((amount, rest))
        } else {
            Err(ProgramError::InvalidInstructionData)
        }
    }

    fn unpack_u128(input: &[u8]) -> Result<(u128, &[u8]), ProgramError> {
        if input.len() >= 16 {
            let (amount, rest) = input.split_at(16);
            let amount = amount
                .get(..16)
                .and_then(|slice| slice.try_into().ok())
                .map(u128::from_le_bytes)
                .ok_or(ProgramError::InvalidInstructionData)?;
            Ok((amount, rest))
        } else {
            Err(ProgramError::InvalidInstructionData)
        }
    }
}