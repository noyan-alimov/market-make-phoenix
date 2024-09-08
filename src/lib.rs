use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey
};

pub mod state;
pub mod instruction;
pub mod processor;
pub mod error;

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);
fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let ixn = instruction::Instruction::unpack(instruction_data)?;
    match ixn {
        instruction::Instruction::CreatePosition(data) => {
            processor::process_create_position(
                program_id,
                accounts,
                data.spread_margin,
                data.side,
                data.num_base_lots,
                data.client_order_id,
            )
        }
        instruction::Instruction::CancelPosition => {
            processor::process_cancel_position(program_id, accounts)
        }
        instruction::Instruction::PlaceLimitOrdersWithFreeFunds(data) => {
            processor::process_place_limit_orders_with_free_funds(program_id, accounts, data.client_order_id)
        }
    }
}