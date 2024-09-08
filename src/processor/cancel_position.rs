use phoenix::program::{create_cancel_all_order_with_free_funds_instruction, create_withdraw_funds_instruction_with_custom_token_accounts};
use solana_program::{account_info::{AccountInfo, next_account_info}, entrypoint::ProgramResult, msg, program::invoke_signed, program_error::ProgramError, program_pack::{IsInitialized, Pack}, pubkey::Pubkey, system_program};
use spl_token::instruction::{close_account, transfer};

use crate::{error::CodeError, state::Position};



pub fn process_cancel_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Cancel position");

    let account_info_iter = &mut accounts.iter();
    let phoenix_program = next_account_info(account_info_iter)?;
    let phoenix_log_authority = next_account_info(account_info_iter)?;
    let market = next_account_info(account_info_iter)?;
    let trader = next_account_info(account_info_iter)?;
    let position = next_account_info(account_info_iter)?;
    let position_base_token_account = next_account_info(account_info_iter)?;
    let position_quote_token_account = next_account_info(account_info_iter)?;
    let base_vault = next_account_info(account_info_iter)?;
    let quote_vault = next_account_info(account_info_iter)?;
    let trader_base_token_account = next_account_info(account_info_iter)?;
    let trader_quote_token_account = next_account_info(account_info_iter)?;
    let base_mint = next_account_info(account_info_iter)?;
    let quote_mint = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;

    check_accounts(
        phoenix_program,
        trader,
        position,
        position_base_token_account,
        position_quote_token_account,
        base_vault,
        quote_vault,
        trader_base_token_account,
        trader_quote_token_account,
        base_mint,
        quote_mint,
        token_program,
        system_program,
    )?;

    let (position_pubkey, position_bump) = Pubkey::find_program_address(&[Position::SEED.as_bytes(), trader.key.as_ref(), market.key.as_ref()], program_id);
    let (position_base_token_account_pubkey, _position_base_token_account_bump) = Pubkey::find_program_address(&[Position::BASE_TOKEN_SEED.as_bytes(), position_pubkey.as_ref(), base_mint.key.as_ref()], program_id);
    let (position_quote_token_account_pubkey, _position_quote_token_account_bump) = Pubkey::find_program_address(&[Position::QUOTE_TOKEN_SEED.as_bytes(), position_pubkey.as_ref(), quote_mint.key.as_ref()], program_id);
    if position.key != &position_pubkey || position_base_token_account.key != &position_base_token_account_pubkey || position_quote_token_account.key != &position_quote_token_account_pubkey {
        msg!("Invalid position, base token account or quote token account");
        return Err(ProgramError::InvalidAccountData);
    }

    cancel_orders_and_withdraw_funds_from_phoenix_to_position(
        phoenix_program,
        phoenix_log_authority,
        market,
        trader,
        position,
        position_base_token_account,
        position_quote_token_account,
        base_vault,
        quote_vault,
        base_mint,
        quote_mint,
        position_bump
    )?;

    withdraw_and_close_position_and_token_accounts(
        trader,
        market,
        position,
        position_base_token_account,
        position_quote_token_account,
        trader_base_token_account,
        trader_quote_token_account,
        token_program,
        position_bump
    )?;

    Ok(())
}


fn check_accounts(
    phoenix_program: &AccountInfo,
    trader: &AccountInfo,
    position: &AccountInfo,
    position_base_token_account: &AccountInfo,
    position_quote_token_account: &AccountInfo,
    base_vault: &AccountInfo,
    quote_vault: &AccountInfo,
    trader_base_token_account: &AccountInfo,
    trader_quote_token_account: &AccountInfo,
    base_mint: &AccountInfo,
    quote_mint: &AccountInfo,
    token_program: &AccountInfo,
    system_program: &AccountInfo,
) -> ProgramResult {
    if phoenix_program.key.to_string() != "PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY" {
        msg!("Invalid phoenix program account");
        return Err(ProgramError::InvalidAccountData);
    }

    if !trader.is_signer || !trader.is_writable {
        msg!("Trader account should be signer and writable");
        return Err(ProgramError::InvalidAccountData);
    }

    if !position.is_writable || !position_base_token_account.is_writable || !position_quote_token_account.is_writable {
        msg!("Position, base token account and quote token account should be writable");
        return Err(ProgramError::InvalidAccountData);
    }

    if !trader_base_token_account.is_writable || !trader_quote_token_account.is_writable {
        msg!("Trader base and quote token accounts should be writable");
        return Err(ProgramError::InvalidAccountData);
    }

    if token_program.key != &spl_token::id() {
        msg!("Invalid token program account");
        return Err(ProgramError::InvalidAccountData);
    }

    if system_program.key != &system_program::id() {
        msg!("Invalid system program account");
        return Err(ProgramError::InvalidAccountData);
    }

    let trader_base_token_account_data = spl_token::state::Account::unpack(&trader_base_token_account.data.borrow())?;
    let trader_quote_token_account_data = spl_token::state::Account::unpack(&trader_quote_token_account.data.borrow())?;
    if trader_base_token_account_data.owner != *trader.key || trader_base_token_account_data.mint != *base_mint.key {
        msg!("Invalid trader base token account");
        return Err(ProgramError::InvalidAccountData);
    }
    if trader_quote_token_account_data.owner != *trader.key || trader_quote_token_account_data.mint != *quote_mint.key {
        msg!("Invalid trader quote token account");
        return Err(ProgramError::InvalidAccountData);
    }

    let base_vault_data = spl_token::state::Account::unpack(&base_vault.data.borrow())?;
    let quote_vault_data = spl_token::state::Account::unpack(&quote_vault.data.borrow())?;
    if base_vault_data.mint != *base_mint.key || quote_vault_data.mint != *quote_mint.key {
        msg!("Invalid base or quote vault account");
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

fn cancel_orders_and_withdraw_funds_from_phoenix_to_position<'a>(
    phoenix_program: &AccountInfo<'a>,
    phoenix_log_authority: &AccountInfo<'a>,
    market: &AccountInfo<'a>,
    trader: &AccountInfo<'a>,
    position: &AccountInfo<'a>,
    position_base_token_account: &AccountInfo<'a>,
    position_quote_token_account: &AccountInfo<'a>,
    base_vault: &AccountInfo<'a>,
    quote_vault: &AccountInfo<'a>,
    base_mint: &AccountInfo<'a>,
    quote_mint: &AccountInfo<'a>,
    position_bump: u8
) -> ProgramResult {
    let cancel_limit_order_ixn = create_cancel_all_order_with_free_funds_instruction(market.key, trader.key);
    invoke_signed(
        &cancel_limit_order_ixn,
        &[phoenix_program.clone(), phoenix_log_authority.clone(), market.clone(), trader.clone(), phoenix_program.clone()],
        &[&[Position::SEED.as_bytes(), trader.key.as_ref(), market.key.as_ref(), &[position_bump]],
    ])?;

    let withdraw_all_funds_ixn = create_withdraw_funds_instruction_with_custom_token_accounts(
        market.key,
        position.key,
        position_base_token_account.key,
        position_quote_token_account.key,
        base_mint.key,
        quote_mint.key
    );
    invoke_signed(
        &withdraw_all_funds_ixn,
        &[phoenix_program.clone(), phoenix_log_authority.clone(), market.clone(), position.clone(), position_base_token_account.clone(), position_quote_token_account.clone(), base_vault.clone(), quote_vault.clone(), phoenix_program.clone()],
        &[&[Position::SEED.as_bytes(), trader.key.as_ref(), market.key.as_ref(), &[position_bump]],
    ])?;

    Ok(())
}

fn withdraw_and_close_position_and_token_accounts<'a>(
    trader: &AccountInfo<'a>,
    market: &AccountInfo<'a>,
    position: &AccountInfo<'a>,
    position_base_token_account: &AccountInfo<'a>,
    position_quote_token_account: &AccountInfo<'a>,
    trader_base_token_account: &AccountInfo<'a>,
    trader_quote_token_account: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    position_bump: u8
) -> ProgramResult {
    let position_data = Position::unpack(&position.data.borrow())?;
    if !position_data.is_initialized() {
        msg!("Position is not initialized");
        return Err(CodeError::PositionNotInitialized.into());
    }

    let position_base_token_account_data = spl_token::state::Account::unpack(&position_base_token_account.data.borrow())?;
    let transfer_base_tokens_ixn = transfer(token_program.key, position_base_token_account.key, trader_base_token_account.key, position.key, &[position.key], position_base_token_account_data.amount)?;
    invoke_signed(
        &transfer_base_tokens_ixn,
        &[position_base_token_account.clone(), trader_base_token_account.clone(), position.clone(), token_program.clone()],
        &[&[Position::SEED.as_bytes(), trader.key.as_ref(), market.key.as_ref(), &[position_bump]]]
    )?;

    let position_quote_token_account_data = spl_token::state::Account::unpack(&position_quote_token_account.data.borrow())?;
    let transfer_quote_tokens_ixn = transfer(token_program.key, position_quote_token_account.key, trader_quote_token_account.key, position.key, &[position.key], position_quote_token_account_data.amount)?;
    invoke_signed(
        &transfer_quote_tokens_ixn,
        &[position_quote_token_account.clone(), trader_quote_token_account.clone(), position.clone(), token_program.clone()],
        &[&[Position::SEED.as_bytes(), trader.key.as_ref(), market.key.as_ref(), &[position_bump]]]
    )?;

    let close_position_base_token_account_ixn = close_account(token_program.key, position_base_token_account.key, trader.key, position.key, &[position.key])?;
    invoke_signed(
        &close_position_base_token_account_ixn,
        &[position_base_token_account.clone(), trader.clone(), position.clone(), token_program.clone()],
        &[&[Position::SEED.as_bytes(), trader.key.as_ref(), market.key.as_ref(), &[position_bump]]]
    )?;

    let close_position_quote_token_account_ixn = close_account(token_program.key, position_quote_token_account.key, trader.key, position.key, &[position.key])?;
    invoke_signed(
        &close_position_quote_token_account_ixn,
        &[position_quote_token_account.clone(), trader.clone(), position.clone(), token_program.clone()],
        &[&[Position::SEED.as_bytes(), trader.key.as_ref(), market.key.as_ref(), &[position_bump]]]
    )?;

    **trader.try_borrow_mut_lamports()? = trader
        .lamports()
        .checked_add(position.lamports())
        .ok_or(CodeError::ArithmeticOverflow)?;
    **position.try_borrow_mut_lamports()? = 0;
    *position.try_borrow_mut_data()? = &mut [];

    Ok(())
}
