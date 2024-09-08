use phoenix::{program::{create_new_order_instruction_with_custom_token_accounts, load_with_dispatch, MarketHeader}, state::{OrderPacket, SelfTradeBehavior, Side}};
use solana_program::{account_info::{AccountInfo, next_account_info}, entrypoint::ProgramResult, msg, program::{invoke, invoke_signed}, program_error::ProgramError, program_pack::{IsInitialized, Pack}, pubkey::Pubkey, rent::Rent, system_instruction::create_account, system_program, sysvar::Sysvar};
use spl_token::{state::Account, instruction::{initialize_account3, transfer}};
use core::mem::size_of;

use crate::{error::CodeError, state::Position};


pub fn process_create_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    spread_margin: u64,
    side: u8,
    num_base_lots: u64,
    client_order_id: u128,
) -> ProgramResult {
    msg!("Create position");

    let side_enum = match side {
        1 => Side::Bid,
        2 => Side::Ask,
        _ => {
            msg!("Invalid side");
            return Err(ProgramError::InvalidInstructionData);
        }
    };

    if spread_margin > 100 || spread_margin == 0 {
        msg!("Invalid spread margin");
        return Err(ProgramError::InvalidInstructionData);
    }

    let account_info_iter = &mut accounts.iter();
    let phoenix_program = next_account_info(account_info_iter)?;
    let phoenix_log_authority = next_account_info(account_info_iter)?;
    let market = next_account_info(account_info_iter)?;
    let trader = next_account_info(account_info_iter)?;
    let seat = next_account_info(account_info_iter)?;
    let position = next_account_info(account_info_iter)?;
    let position_base_token_account = next_account_info(account_info_iter)?;
    let position_quote_token_account = next_account_info(account_info_iter)?;
    let base_vault = next_account_info(account_info_iter)?;
    let quote_vault = next_account_info(account_info_iter)?;
    let base_mint = next_account_info(account_info_iter)?;
    let quote_mint = next_account_info(account_info_iter)?;
    let trader_base_token_account = next_account_info(account_info_iter)?;
    let trader_quote_token_account = next_account_info(account_info_iter)?;
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
        base_mint,
        quote_mint,
        trader_base_token_account,
        trader_quote_token_account,
        token_program,
        system_program
    )?;

    let (position_pubkey, position_bump) = Pubkey::find_program_address(&[Position::SEED.as_bytes(), trader.key.as_ref(), market.key.as_ref()], program_id);
    let (position_base_token_account_pubkey, position_base_token_account_bump) = Pubkey::find_program_address(&[Position::BASE_TOKEN_SEED.as_bytes(), position_pubkey.as_ref(), base_mint.key.as_ref()], program_id);
    let (position_quote_token_account_pubkey, position_quote_token_account_bump) = Pubkey::find_program_address(&[Position::QUOTE_TOKEN_SEED.as_bytes(), position_pubkey.as_ref(), quote_mint.key.as_ref()], program_id);
    if position.key != &position_pubkey || position_base_token_account.key != &position_base_token_account_pubkey || position_quote_token_account.key != &position_quote_token_account_pubkey {
        msg!("Invalid position, base token account or quote token account");
        return Err(ProgramError::InvalidAccountData);
    }

    let rent = Rent::get()?;

    create_position_account(rent, trader.clone(), position.clone(), position_bump, market.clone(), system_program.clone(), spread_margin, program_id)?;

    create_and_initialize_position_token_accounts(
        rent,
        trader.clone(),
        &position_pubkey,
        position_base_token_account.clone(),
        position_base_token_account_bump,
        position_quote_token_account.clone(),
        position_quote_token_account_bump,
        base_mint.clone(),
        quote_mint.clone(),
        token_program.clone()
    )?;

    let market_account_data = market.data.borrow();
    let (header_bytes, market_bytes) = market_account_data.split_at(size_of::<MarketHeader>());
    let (
        bid_price,
        bid_quote_tokens_to_transfer,
        ask_price,
        ask_base_tokens_to_transfer
    ) = get_market_data(header_bytes, market_bytes, spread_margin, num_base_lots);

    transfer_tokens_to_position(
        side_enum,
        bid_quote_tokens_to_transfer,
        ask_base_tokens_to_transfer,
        trader.clone(),
        position_base_token_account.clone(),
        position_quote_token_account.clone(),
        trader_base_token_account.clone(),
        trader_quote_token_account.clone(),
        token_program.clone()
    )?;

    place_limit_order_on_phoenix(
        side_enum,
        bid_price,
        ask_price,
        num_base_lots,
        client_order_id,
        trader.clone(),
        position.clone(),
        position_bump,
        market.clone(),
        seat.clone(),
        position_base_token_account.clone(),
        position_quote_token_account.clone(),
        base_vault.clone(),
        quote_vault.clone(),
        base_mint.clone(),
        quote_mint.clone(),
        token_program.clone(),
        phoenix_program.clone(),
        phoenix_log_authority.clone()
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
    base_mint: &AccountInfo,
    quote_mint: &AccountInfo,
    trader_base_token_account: &AccountInfo,
    trader_quote_token_account: &AccountInfo,
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

fn create_position_account<'a>(
    rent: Rent,
    trader: AccountInfo<'a>,
    position: AccountInfo<'a>,
    position_bump: u8,
    market: AccountInfo<'a>,
    system_program: AccountInfo<'a>,
    spread_margin: u64,
    program_id: &Pubkey
) -> ProgramResult {
    let position_size = 8 + std::mem::size_of::<Position>();
    let lamports = rent.minimum_balance(position_size);
    let create_position_ixn = create_account(trader.key, position.key, lamports, position_size.try_into().unwrap(), program_id);
    invoke_signed(
        &create_position_ixn,
        &[trader.clone(), position.clone(), system_program],
        &[&[Position::SEED.as_bytes(), trader.key.as_ref(), market.key.as_ref(), &[position_bump]]]
    )?;
    let mut position_data_bytes = position.data.borrow_mut();
    let mut position_data = Position::unpack(&position_data_bytes)?;
    if position_data.is_initialized() {
        msg!("Position is already initialized");
        return Err(CodeError::PositionIsAlreadyInitialized.into());
    }

    position_data.is_initialized = true;
    position_data.spread_margin = spread_margin;
    Position::pack(position_data, &mut position_data_bytes)?;

    Ok(())
}

fn create_and_initialize_position_token_accounts<'a>(
    rent: Rent,
    trader: AccountInfo<'a>,
    position_pubkey: &Pubkey,
    position_base_token_account: AccountInfo<'a>,
    position_base_token_account_bump: u8,
    position_quote_token_account: AccountInfo<'a>,
    position_quote_token_account_bump: u8,
    base_mint: AccountInfo<'a>,
    quote_mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
) -> ProgramResult {
    let token_account_size = Account::LEN;
    let lamports = rent.minimum_balance(token_account_size);

    let create_position_base_token_account_ixn = create_account(trader.key, position_base_token_account.key, lamports, token_account_size.try_into().unwrap(), token_program.key);
    invoke_signed(
        &create_position_base_token_account_ixn,
        &[trader.clone(), position_base_token_account.clone(), token_program.clone()],
        &[&[Position::BASE_TOKEN_SEED.as_bytes(), position_pubkey.as_ref(), base_mint.key.as_ref(), &[position_base_token_account_bump]]]
    )?;
    let initialize_position_base_token_account_ixn = initialize_account3(token_program.key, position_base_token_account.key, base_mint.key, &position_pubkey)?;
    invoke(
        &initialize_position_base_token_account_ixn, 
        &[position_base_token_account.clone(), base_mint.clone(), token_program.clone()]
    )?;

    let create_position_quote_token_account_ixn = create_account(trader.key, position_quote_token_account.key, lamports, token_account_size.try_into().unwrap(), token_program.key);
    invoke_signed(
        &create_position_quote_token_account_ixn,
        &[trader.clone(), position_quote_token_account.clone(), token_program.clone()],
        &[&[Position::QUOTE_TOKEN_SEED.as_bytes(), position_pubkey.as_ref(), quote_mint.key.as_ref(), &[position_quote_token_account_bump]]]
    )?;
    let initialize_position_quote_token_account_ixn = initialize_account3(token_program.key, position_quote_token_account.key, quote_mint.key, &position_pubkey)?;
    invoke(
        &initialize_position_quote_token_account_ixn, 
        &[position_quote_token_account.clone(), quote_mint.clone(), token_program.clone()]
    )?;

    Ok(())
}

fn get_market_data(header_bytes: &[u8], market_bytes: &[u8], spread_margin: u64, num_base_lots: u64) -> (u64, u64, u64, u64) {
    let header = bytemuck::try_from_bytes::<MarketHeader>(header_bytes).unwrap();
    let market_decoded_data = load_with_dispatch(&header.market_size_params, market_bytes).unwrap().inner;
    let ladder = market_decoded_data.get_ladder(1);
    let max_bid_price = ladder.bids.get(0).unwrap().price_in_ticks;
    let min_ask_price = ladder.asks.get(0).unwrap().price_in_ticks;
    let market_price = (max_bid_price + min_ask_price) / 2;
    
    let bid_price = market_price * ((100 - spread_margin) / 100);
    let bid_quote_tokens_to_transfer = num_base_lots * bid_price;
    let ask_price = market_price * ((100 + spread_margin) / 100);
    let base_lots_per_base_unit: u64 = market_decoded_data.get_base_lots_per_base_unit().into();
    let ask_base_tokens_to_transfer = num_base_lots * base_lots_per_base_unit;

    return (bid_price, bid_quote_tokens_to_transfer, ask_price, ask_base_tokens_to_transfer);
}

fn transfer_tokens_to_position<'a>(
    side_enum: Side,
    bid_quote_tokens_to_transfer: u64,
    ask_base_tokens_to_transfer: u64,
    trader: AccountInfo<'a>,
    position_base_token_account: AccountInfo<'a>,
    position_quote_token_account: AccountInfo<'a>,
    trader_base_token_account: AccountInfo<'a>,
    trader_quote_token_account: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
) -> ProgramResult {
    match side_enum {
        Side::Bid => {
            let transfer_quote_tokens_ixn = transfer(
                token_program.key,
                trader_quote_token_account.key,
                position_quote_token_account.key,
                trader.key,
                &[trader.key],
                bid_quote_tokens_to_transfer
            )?;
            invoke(
                &transfer_quote_tokens_ixn,
                &[trader_quote_token_account.clone(), position_quote_token_account.clone(), trader.clone(), token_program.clone()]
            )?;
        },
        Side::Ask => {
            let transfer_base_tokens_ixn = transfer(
                token_program.key,
                trader_base_token_account.key,
                position_base_token_account.key,
                trader.key,
                &[trader.key],
                ask_base_tokens_to_transfer
            )?;
            invoke(
                &transfer_base_tokens_ixn,
                &[trader_base_token_account.clone(), position_base_token_account.clone(), trader.clone(), token_program.clone()]
            )?;
        }
    }

    Ok(())
}

fn place_limit_order_on_phoenix<'a>(
    side_enum: Side,
    bid_price_in_ticks: u64,
    ask_price_in_ticks: u64,
    num_base_lots: u64,
    client_order_id: u128,
    trader: AccountInfo<'a>,
    position: AccountInfo<'a>,
    position_bump: u8,
    market: AccountInfo<'a>,
    seat: AccountInfo<'a>,
    position_base_token_account: AccountInfo<'a>,
    position_quote_token_account: AccountInfo<'a>,
    base_vault: AccountInfo<'a>,
    quote_vault: AccountInfo<'a>,
    base_mint: AccountInfo<'a>,
    quote_mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    phoenix_program: AccountInfo<'a>,
    phoenix_log_authority: AccountInfo<'a>,
) -> ProgramResult {
    let order_packet = OrderPacket::new_limit_order(
        side_enum,
        match side_enum {
            Side::Bid => bid_price_in_ticks,
            Side::Ask => ask_price_in_ticks,
        },
        num_base_lots,
        SelfTradeBehavior::CancelProvide,
        None,
        client_order_id,
        false
    );
    let place_limit_order_ixn = create_new_order_instruction_with_custom_token_accounts(
        market.key,
        position.key,
        position_base_token_account.key,
        position_quote_token_account.key,
        base_mint.key,
        quote_mint.key,
        &order_packet
    );
    invoke_signed(
        &place_limit_order_ixn,
        &[
            phoenix_program.clone(),
            phoenix_log_authority.clone(),
            market.clone(),
            position.clone(),
            seat.clone(),
            position_base_token_account.clone(),
            position_quote_token_account.clone(),
            base_vault.clone(),
            quote_vault.clone(),
            token_program.clone(),
            phoenix_program.clone(),
        ],
        &[&[Position::SEED.as_bytes(), trader.key.as_ref(), market.key.as_ref(), &[position_bump]]],
    )?;

    Ok(())
}