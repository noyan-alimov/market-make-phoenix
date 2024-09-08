use phoenix::{program::{create_new_order_with_free_funds_instruction, load_with_dispatch, MarketHeader}, quantities::{BaseLots, Ticks, WrapperU64}, state::{OrderPacket, SelfTradeBehavior, Side}};
use solana_program::{account_info::{AccountInfo, next_account_info}, entrypoint::ProgramResult, msg, program::invoke_signed, program_error::ProgramError, program_pack::Pack, pubkey::Pubkey, system_program};
use core::mem::size_of;

use crate::state::Position;


pub fn process_place_limit_orders_with_free_funds(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    client_order_id: u128
) -> ProgramResult {
    msg!("Place limit orders with free funds");

    let account_info_iter = &mut accounts.iter();
    let phoenix_program = next_account_info(account_info_iter)?;
    let phoenix_log_authority = next_account_info(account_info_iter)?;
    let market = next_account_info(account_info_iter)?;
    let trader = next_account_info(account_info_iter)?;
    let seat = next_account_info(account_info_iter)?;
    let position = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;

    check_accounts(
        phoenix_program,
        position,
        system_program,
    )?;

    let (position_pubkey, position_bump) = Pubkey::find_program_address(&[Position::SEED.as_bytes(), trader.key.as_ref(), market.key.as_ref()], program_id);
    if position.key != &position_pubkey {
        msg!("Invalid position account");
        return Err(ProgramError::InvalidAccountData);
    }

    let position_data_bytes = position.data.borrow();
    let position_data = Position::unpack(&position_data_bytes)?;
    let spread_margin = position_data.spread_margin;

    let market_account_data = market.data.borrow();
    let (header_bytes, market_bytes) = market_account_data.split_at(size_of::<MarketHeader>());
    let header = bytemuck::try_from_bytes::<MarketHeader>(header_bytes).unwrap();
    let market_decoded_data = load_with_dispatch(&header.market_size_params, market_bytes).unwrap().inner;

    let trader_state = market_decoded_data.get_trader_state(position.key).unwrap();

    let ladder = market_decoded_data.get_ladder(1);
    let max_bid_price = ladder.bids.get(0).unwrap().price_in_ticks;
    let min_ask_price = ladder.asks.get(0).unwrap().price_in_ticks;
    let market_price = (max_bid_price + min_ask_price) / 2;
    
    let bid_price = market_price * ((100 - spread_margin) / 100);
    let ask_price = market_price * ((100 + spread_margin) / 100);

    // place bid limit order
    if Into::<u64>::into(trader_state.quote_lots_free) > 0 {
        let bid_order_packet = OrderPacket::Limit {
            side: Side::Bid,
            price_in_ticks: Ticks::new(bid_price),
            num_base_lots: BaseLots::new(trader_state.quote_lots_free.as_u64() / bid_price),
            self_trade_behavior: SelfTradeBehavior::CancelProvide,
            match_limit: None,
            client_order_id,
            use_only_deposited_funds: true,
            last_valid_slot: None,
            last_valid_unix_timestamp_in_seconds: None,
            fail_silently_on_insufficient_funds: true,
        };
        let bid_place_limit_order_ixn = create_new_order_with_free_funds_instruction(
            market.key,
            position.key,
            &bid_order_packet
        );
        invoke_signed(
            &bid_place_limit_order_ixn,
            &[
                phoenix_program.clone(),
                phoenix_log_authority.clone(),
                market.clone(),
                position.clone(),
                seat.clone(),
                phoenix_program.clone(),
            ],
            &[&[Position::SEED.as_bytes(), trader.key.as_ref(), market.key.as_ref(), &[position_bump]]],
        )?;
    }

    // place ask limit order
    if Into::<u64>::into(trader_state.base_lots_free) > 0 {
        let ask_order_packet = OrderPacket::Limit {
            side: Side::Ask,
            price_in_ticks: Ticks::new(ask_price),
            num_base_lots: trader_state.base_lots_free,
            self_trade_behavior: SelfTradeBehavior::CancelProvide,
            match_limit: None,
            client_order_id,
            use_only_deposited_funds: true,
            last_valid_slot: None,
            last_valid_unix_timestamp_in_seconds: None,
            fail_silently_on_insufficient_funds: true,
        };
        let ask_place_limit_order_ixn = create_new_order_with_free_funds_instruction(
            market.key,
            position.key,
            &ask_order_packet
        );
        invoke_signed(
            &ask_place_limit_order_ixn,
            &[
                phoenix_program.clone(),
                phoenix_log_authority.clone(),
                market.clone(),
                position.clone(),
                seat.clone(),
                phoenix_program.clone(),
            ],
            &[&[Position::SEED.as_bytes(), trader.key.as_ref(), market.key.as_ref(), &[position_bump]]],
        )?;
    }
    
    Ok(())
}

fn check_accounts(
    phoenix_program: &AccountInfo,
    position: &AccountInfo,
    system_program: &AccountInfo,
) -> ProgramResult {
    if phoenix_program.key.to_string() != "PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY" {
        msg!("Invalid phoenix program account");
        return Err(ProgramError::InvalidAccountData);
    }

    if !position.is_writable {
        msg!("Position account should be writable");
        return Err(ProgramError::InvalidAccountData);
    }

    if system_program.key != &system_program::id() {
        msg!("Invalid system program account");
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}