use anchor_lang::{prelude::*, system_program};
use crate::put_options::validators::*;
use crate::put_options::errors::PutOptionError;
use crate::{
    FREEZE_SECONDS, 
    LAMPORTS_FOR_UPDATE_SETTLEPRICE_TICKET, 
    LAMPORTS_FOR_UPDATE_FAIRPRICE_TICKET,
    MAX_MATURITY_FUTURE_SECONDS,
    EMERGENCY_MODE_GRACE_PERIOD,
    MAX_SECONDS_FROM_LAST_FAIR_PRICE_UPDATE,
    PROTOCOL_TOTAL_FEES,
    FRONTEND_SHARE
};
use crate::MakerCreatePutOptionParams;
use anchor_spl::token::{self, Transfer, TokenAccount};
use crate::anchor_solhedge::*;
use crate::put_options::data::PutOptionMakerInfo;

pub fn oracle_update_put_option_settle_price(
    ctx: Context<OracleUpdatePutOptionSettlePrice>,
    settle_price: u64
) -> Result<()> {
    require!(
        settle_price > 0,
        PutOptionError::PriceZero
    );
    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        ctx.accounts.vault_factory_info.maturity < current_time,
        PutOptionError::MaturityTooLate
    );

    if !ctx.accounts.vault_factory_info.matured {
        ctx.accounts.vault_factory_info.settled_price = settle_price;
        ctx.accounts.vault_factory_info.matured = true;
    }

    ctx.accounts.update_ticket.is_used = true;

    Ok(())

}

pub fn oracle_update_put_option_price(
    ctx: Context<OracleUpdatePutOptionFairPrice>,
    new_fair_price: u64
) -> Result<()> {
    require!(
        new_fair_price > 0,
        PutOptionError::PriceZero
    );

    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    if ctx.accounts.vault_factory_info.maturity > current_time.checked_add(FREEZE_SECONDS).unwrap() {
        ctx.accounts.vault_factory_info.last_fair_price = new_fair_price;
        ctx.accounts.vault_factory_info.ts_last_fair_price = current_time;
    }
    ctx.accounts.update_ticket.is_used = true;
    Ok(())
}

pub fn gen_settle_put_option_price_ticket(ctx: Context<GenSettlePutOptionPriceTicket>) -> Result<()> {
    require!(
        ctx.accounts.put_option_settle_price_ticket.is_used == false,
        PutOptionError::UsedUpdateTicket
    );
    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        ctx.accounts.vault_factory_info.maturity < current_time,
        PutOptionError::MaturityTooLate
    );

    msg!("Started transferring lamports to oracle");
    let oracle_fee_transfer_cpi_context = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.initializer.to_account_info(),
            to: ctx.accounts.oracle_wallet.to_account_info()
        }
    );
    system_program::transfer(oracle_fee_transfer_cpi_context, LAMPORTS_FOR_UPDATE_SETTLEPRICE_TICKET)?;
    msg!("Finished transferring lamports to oracle");


    Ok(())
}

pub fn gen_update_put_option_fair_price_ticket(ctx: Context<GenUpdatePutOptionFairPriceTicket>) -> Result<()> {
    require!(
        ctx.accounts.put_option_fair_price_ticket.is_used == false,
        PutOptionError::UsedUpdateTicket
    );

    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        ctx.accounts.vault_factory_info.maturity > current_time.checked_add(FREEZE_SECONDS).unwrap(),
        PutOptionError::MaturityTooEarly
    );


    msg!("Started transferring lamports to oracle");
    let oracle_fee_transfer_cpi_context = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.initializer.to_account_info(),
            to: ctx.accounts.oracle_wallet.to_account_info()
        }
    );
    system_program::transfer(oracle_fee_transfer_cpi_context, LAMPORTS_FOR_UPDATE_FAIRPRICE_TICKET)?;
    msg!("Finished transferring lamports to oracle");

    Ok(())
}

pub fn maker_next_put_option_vault_id(ctx: Context<MakerNextPutOptionVaultId>,
    params: MakerCreatePutOptionParams
) -> Result<u64> {

    require!(
        params.strike > 0,
        PutOptionError::StrikeZero
    );

    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        params.maturity > current_time.checked_add(FREEZE_SECONDS).unwrap(),
        PutOptionError::MaturityTooEarly
    );

    require!(
        params.maturity <= current_time.checked_add(MAX_MATURITY_FUTURE_SECONDS).unwrap(),
        PutOptionError::MaturityTooLate
    );


    // Initializing factory vault (PutOptionVaultFactoryInfo) if it has been just created
    if !ctx.accounts.vault_factory_info.is_initialized {
        ctx.accounts.vault_factory_info.next_vault_id = 1;
        ctx.accounts.vault_factory_info.maturity = params.maturity;
        ctx.accounts.vault_factory_info.matured = false;
        ctx.accounts.vault_factory_info.strike = params.strike;
        ctx.accounts.vault_factory_info.base_asset = ctx.accounts.base_asset_mint.key();
        ctx.accounts.vault_factory_info.quote_asset = ctx.accounts.quote_asset_mint.key();
        ctx.accounts.vault_factory_info.emergency_mode = false;

        ctx.accounts.vault_factory_info.is_initialized = true;
        msg!("PutOptionVaultFactoryInfo initialized");
    }
    let result = ctx.accounts.vault_factory_info.next_vault_id;
    ctx.accounts.vault_factory_info.next_vault_id = ctx.accounts.vault_factory_info.next_vault_id.checked_add(1).unwrap();

    Ok(result)
}

pub fn maker_activate_put_option_emergency_mode(ctx: Context<MakerActivatePutOptionEmergencyMode>) -> Result<()> {
    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        current_time.checked_sub(ctx.accounts.vault_factory_info.maturity).unwrap() > EMERGENCY_MODE_GRACE_PERIOD,
        PutOptionError::EmergencyModeTooEarly
    );
    
    ctx.accounts.vault_factory_info.emergency_mode = true;

    Ok(())
}

pub fn taker_put_option_emergency_exit(ctx: Context<TakerPutOptionEmergencyExit>) -> Result<()> {
    if ctx.accounts.put_option_taker_info.qty_deposited > 0 {
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = Transfer {
            from: ctx.accounts.vault_base_asset_treasury.to_account_info(),
            to: ctx.accounts.taker_base_asset_account.to_account_info(),
            authority: ctx.accounts.vault_info.to_account_info(),
        };

        // Preparing PDA signer
        let auth_bump = *ctx.bumps.get("vault_info").unwrap();
        let seeds = &[
            "PutOptionVaultInfo".as_bytes().as_ref(), 
            &ctx.accounts.vault_factory_info.key().to_bytes(),
            &ctx.accounts.vault_info.ord.to_le_bytes(),
            &[auth_bump],
        ];
        let signer = &[&seeds[..]];


        let token_transfer_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

        token::transfer(token_transfer_context, ctx.accounts.put_option_taker_info.qty_deposited)?;
    }
    ctx.accounts.put_option_taker_info.qty_deposited = 0;
    ctx.accounts.put_option_taker_info.is_settled = true;

    Ok(())
}    

pub fn maker_put_option_emergency_exit(ctx: Context<MakerPutOptionEmergencyExit>) -> Result<()> {
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_accounts = Transfer {
        from: ctx.accounts.vault_quote_asset_treasury.to_account_info(),
        to: ctx.accounts.maker_quote_asset_account.to_account_info(),
        authority: ctx.accounts.vault_info.to_account_info(),
    };

    // Preparing PDA signer
    let auth_bump = *ctx.bumps.get("vault_info").unwrap();
    let seeds = &[
        "PutOptionVaultInfo".as_bytes().as_ref(), 
        &ctx.accounts.vault_factory_info.key().to_bytes(),
        &ctx.accounts.vault_info.ord.to_le_bytes(),
        &[auth_bump],
    ];
    let signer = &[&seeds[..]];


    let token_transfer_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

    token::transfer(token_transfer_context, ctx.accounts.put_option_maker_info.quote_asset_qty)?;

    ctx.accounts.put_option_maker_info.quote_asset_qty = 0;
    ctx.accounts.put_option_maker_info.volume_sold = 0;
    ctx.accounts.put_option_maker_info.is_settled = true;


    Ok(())
}

pub fn taker_activate_put_option_emergency_mode(ctx: Context<TakerActivatePutOptionEmergencyMode>) -> Result<()> {
    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        current_time.checked_sub(ctx.accounts.vault_factory_info.maturity).unwrap() > EMERGENCY_MODE_GRACE_PERIOD,
        PutOptionError::EmergencyModeTooEarly
    );
    
    ctx.accounts.vault_factory_info.emergency_mode = true;

    Ok(())
}

pub fn maker_settle_put_option(ctx: Context<MakerSettlePutOption>) -> Result<PutOptionSettleReturn> {
    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        ctx.accounts.vault_factory_info.maturity < current_time,
        PutOptionError::IllegalState  // should not have passed maturity test, must never happen
    );


    let mut result = PutOptionSettleReturn {
        base_asset_transfer: 0,
        quote_asset_transfer: 0,
        settle_result: PutOptionSettleResult::NotExercised
    };

    if ctx.accounts.vault_factory_info.settled_price > ctx.accounts.vault_factory_info.strike {
        msg!("Put option is not favorable to taker, will NOT be exercised");
        // i.e. maker gets her deposited quote assets back
        result.settle_result = PutOptionSettleResult::NotExercised;
        // Proceed to transfer 
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = Transfer {
            from: ctx.accounts.vault_quote_asset_treasury.to_account_info(),
            to: ctx.accounts.maker_quote_asset_account.to_account_info(),
            authority: ctx.accounts.vault_info.to_account_info(),
        };

        // Preparing PDA signer
        let auth_bump = *ctx.bumps.get("vault_info").unwrap();
        let seeds = &[
            "PutOptionVaultInfo".as_bytes().as_ref(), 
            &ctx.accounts.vault_factory_info.key().to_bytes(),
            &ctx.accounts.vault_info.ord.to_le_bytes(),
            &[auth_bump],
        ];
        let signer = &[&seeds[..]];


        let token_transfer_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

        token::transfer(token_transfer_context, ctx.accounts.put_option_maker_info.quote_asset_qty)?;

        result.quote_asset_transfer = ctx.accounts.put_option_maker_info.quote_asset_qty;
        result.base_asset_transfer = 0;
    } else {
        msg!("Put option is favorable to taker, WILL be exercised");
        // maker will sell up to the limit of ctx.accounts.put_option_maker_info.volume_sold
        // however as takers may have insufficiently funded their options, the maker
        // may eventually sell less, in a first settle first served base

        let total_deposited_quote_lamports_value_f64 = (ctx.accounts.vault_info.takers_total_deposited as f64) / 10.0f64.powf(ctx.accounts.base_asset_mint.decimals as f64) * (ctx.accounts.vault_factory_info.strike as f64);
        require!(
            total_deposited_quote_lamports_value_f64.is_finite(),
            PutOptionError::Overflow
        );
        let total_deposited_quote_lamports_value = total_deposited_quote_lamports_value_f64.floor() as u64;
        let total_bonus = ctx.accounts.vault_info.makers_total_pending_settle - total_deposited_quote_lamports_value;
        let max_bonus = total_bonus.checked_sub(ctx.accounts.vault_info.bonus_not_exercised).unwrap();

        let maker_bonus = std::cmp::min(max_bonus, ctx.accounts.put_option_maker_info.volume_sold);
        let maker_buy_amount = ctx.accounts.put_option_maker_info.volume_sold.checked_sub(maker_bonus).unwrap();
        let mut transfer_quote_asset = ctx.accounts.put_option_maker_info.quote_asset_qty.checked_sub(ctx.accounts.put_option_maker_info.volume_sold).unwrap(); // initially unsold quote assets
        if maker_bonus > 0 {
            transfer_quote_asset = transfer_quote_asset.checked_add(maker_bonus).unwrap();
            ctx.accounts.vault_info.bonus_not_exercised = ctx.accounts.vault_info.bonus_not_exercised.checked_add(maker_bonus).unwrap();
        }
        if transfer_quote_asset > 0 {
            msg!("Lucky maker! Will only be partially exercised!");
            result.settle_result = PutOptionSettleResult::PartiallyExercised;
            // Proceed to transfer 
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_accounts = Transfer {
                from: ctx.accounts.vault_quote_asset_treasury.to_account_info(),
                to: ctx.accounts.maker_quote_asset_account.to_account_info(),
                authority: ctx.accounts.vault_info.to_account_info(),
            };

            // Preparing PDA signer
            let auth_bump = *ctx.bumps.get("vault_info").unwrap();
            let seeds = &[
                "PutOptionVaultInfo".as_bytes().as_ref(), 
                &ctx.accounts.vault_factory_info.key().to_bytes(),
                &ctx.accounts.vault_info.ord.to_le_bytes(),
                &[auth_bump],
            ];
            let signer = &[&seeds[..]];
    

            let token_transfer_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

            token::transfer(token_transfer_context, transfer_quote_asset)?;
            result.quote_asset_transfer = transfer_quote_asset;
        } else {
            msg!("Maker will be fully exercised!");
            result.settle_result = PutOptionSettleResult::FullyExercised;
            result.quote_asset_transfer = 0;
        }
        if maker_buy_amount > 0 {
            let base_lamports_f64 = (maker_buy_amount as f64) / (ctx.accounts.vault_factory_info.strike as f64) * 10.0f64.powf(ctx.accounts.base_asset_mint.decimals as f64);
            require!(
                base_lamports_f64.is_finite(),
                PutOptionError::Overflow
            );
            let base_lamports = base_lamports_f64.floor() as u64;
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_accounts = Transfer {
                from: ctx.accounts.vault_base_asset_treasury.to_account_info(),
                to: ctx.accounts.maker_base_asset_account.to_account_info(),
                authority: ctx.accounts.vault_info.to_account_info(),
            };

            // Preparing PDA signer
            let auth_bump = *ctx.bumps.get("vault_info").unwrap();
            let seeds = &[
                "PutOptionVaultInfo".as_bytes().as_ref(), 
                &ctx.accounts.vault_factory_info.key().to_bytes(),
                &ctx.accounts.vault_info.ord.to_le_bytes(),
                &[auth_bump],
            ];
            let signer = &[&seeds[..]];
    

            let token_transfer_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

            token::transfer(token_transfer_context, base_lamports)?;
            result.base_asset_transfer = base_lamports;
                
        }            
    }

    ctx.accounts.put_option_maker_info.quote_asset_qty = 0;
    ctx.accounts.put_option_maker_info.volume_sold = 0;
    ctx.accounts.put_option_maker_info.is_settled = true;

    Ok(result)
}

pub fn taker_settle_put_option(ctx: Context<TakerSettlePutOption>) -> Result<PutOptionSettleReturn> {
    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        ctx.accounts.vault_factory_info.maturity < current_time,
        PutOptionError::IllegalState  // should not have passed maturity test, must never happen
    );

    let mut result = PutOptionSettleReturn {
        base_asset_transfer: 0,
        quote_asset_transfer: 0,
        settle_result: PutOptionSettleResult::NotExercised
    };


    if ctx.accounts.vault_factory_info.settled_price > ctx.accounts.vault_factory_info.strike {
        msg!("Put option is not favorable to taker, will NOT be exercised");
        // i.e. taker gets her deposited base assets back
        result.settle_result = PutOptionSettleResult::NotExercised;
        if ctx.accounts.put_option_taker_info.qty_deposited > 0 {
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_accounts = Transfer {
                from: ctx.accounts.vault_base_asset_treasury.to_account_info(),
                to: ctx.accounts.taker_base_asset_account.to_account_info(),
                authority: ctx.accounts.vault_info.to_account_info(),
            };

            // Preparing PDA signer
            let auth_bump = *ctx.bumps.get("vault_info").unwrap();
            let seeds = &[
                "PutOptionVaultInfo".as_bytes().as_ref(), 
                &ctx.accounts.vault_factory_info.key().to_bytes(),
                &ctx.accounts.vault_info.ord.to_le_bytes(),
                &[auth_bump],
            ];
            let signer = &[&seeds[..]];
    

            let token_transfer_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

            token::transfer(token_transfer_context, ctx.accounts.put_option_taker_info.qty_deposited)?;
            result.base_asset_transfer = ctx.accounts.put_option_taker_info.qty_deposited;
            result.quote_asset_transfer = 0;
        }
    } else {
        msg!("Put option is favorable to taker, WILL be exercised");
        // i.e. sell qty_deposited at strike price
        result.settle_result = PutOptionSettleResult::PartiallyExercised;
        if ctx.accounts.put_option_taker_info.qty_deposited > 0 {
            if ctx.accounts.put_option_taker_info.qty_deposited == ctx.accounts.put_option_taker_info.max_base_asset {
                result.settle_result = PutOptionSettleResult::FullyExercised;
            }
            let qty_deposited_quote_lamports_value_f64 = (ctx.accounts.put_option_taker_info.qty_deposited as f64) / 10.0f64.powf(ctx.accounts.base_asset_mint.decimals as f64) * (ctx.accounts.vault_factory_info.strike as f64);
            require!(
                qty_deposited_quote_lamports_value_f64.is_finite(),
                PutOptionError::Overflow
            );
            let qty_deposited_quote_lamports_value = qty_deposited_quote_lamports_value_f64.floor() as u64;
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_accounts = Transfer {
                from: ctx.accounts.vault_quote_asset_treasury.to_account_info(),
                to: ctx.accounts.taker_quote_asset_account.to_account_info(),
                authority: ctx.accounts.vault_info.to_account_info(),
            };

            // Preparing PDA signer
            let auth_bump = *ctx.bumps.get("vault_info").unwrap();
            let seeds = &[
                "PutOptionVaultInfo".as_bytes().as_ref(), 
                &ctx.accounts.vault_factory_info.key().to_bytes(),
                &ctx.accounts.vault_info.ord.to_le_bytes(),
                &[auth_bump],
            ];
            let signer = &[&seeds[..]];
    

            let token_transfer_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

            token::transfer(token_transfer_context, qty_deposited_quote_lamports_value)?;
            result.base_asset_transfer = 0;
            result.quote_asset_transfer = qty_deposited_quote_lamports_value;
        }
    }
    ctx.accounts.put_option_taker_info.qty_deposited = 0;
    ctx.accounts.put_option_taker_info.is_settled = true;

    Ok(result)
}

pub fn taker_adjust_funding_put_option_vault(ctx: Context<TakerAdjustFundingPutOptionVault>,
    new_funding: u64
) -> Result<u64> {

    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    // Period to adjust funding is already closed
    require!(
        ctx.accounts.vault_factory_info.maturity > current_time.checked_add(FREEZE_SECONDS).unwrap(),
        PutOptionError::MaturityTooEarly
    );

    let mut final_funding = ctx.accounts.put_option_taker_info.qty_deposited;
    if new_funding > ctx.accounts.put_option_taker_info.qty_deposited {
        // user wants to increase funding
        let wanted_increase_amount = new_funding.checked_sub(ctx.accounts.put_option_taker_info.qty_deposited).unwrap();
        let max_increase_amount = ctx.accounts.put_option_taker_info.max_base_asset.checked_sub(ctx.accounts.put_option_taker_info.qty_deposited).unwrap();
        let increase_amount = std::cmp::min(wanted_increase_amount, max_increase_amount);
        if increase_amount > 0 {
            {
                let cpi_program = ctx.accounts.token_program.to_account_info();
                msg!("Started transferring base assets to increase funding for option");
                let cpi_accounts = Transfer {
                    from: ctx.accounts.taker_base_asset_account.to_account_info(),
                    to: ctx.accounts.vault_base_asset_treasury.to_account_info(),
                    authority: ctx.accounts.initializer.to_account_info(),
                };
                let token_transfer_context = CpiContext::new(cpi_program, cpi_accounts);
                token::transfer(token_transfer_context, increase_amount)?;
                msg!("Finished transferring base assets to increase funding for option");
            }            
            final_funding = final_funding.checked_add(increase_amount).unwrap();
            ctx.accounts.put_option_taker_info.qty_deposited = final_funding;
            ctx.accounts.vault_info.takers_total_deposited = ctx.accounts.vault_info.takers_total_deposited.checked_add(increase_amount).unwrap();
        }

    } else if new_funding < ctx.accounts.put_option_taker_info.qty_deposited {
        let decrease_amount = ctx.accounts.put_option_taker_info.qty_deposited.checked_sub(new_funding).unwrap();
        // Proceed to transfer 
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = Transfer {
            from: ctx.accounts.vault_base_asset_treasury.to_account_info(),
            to: ctx.accounts.taker_base_asset_account.to_account_info(),
            authority: ctx.accounts.vault_info.to_account_info(),
        };

        // Preparing PDA signer
        let auth_bump = *ctx.bumps.get("vault_info").unwrap();
        let seeds = &[
            "PutOptionVaultInfo".as_bytes().as_ref(), 
            &ctx.accounts.vault_factory_info.key().to_bytes(),
            &ctx.accounts.vault_info.ord.to_le_bytes(),
            &[auth_bump],
        ];
        let signer = &[&seeds[..]];


        let token_transfer_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

        token::transfer(token_transfer_context, decrease_amount)?;
        final_funding = new_funding;
        ctx.accounts.put_option_taker_info.qty_deposited = new_funding;
        ctx.accounts.vault_info.takers_total_deposited = ctx.accounts.vault_info.takers_total_deposited.checked_sub(decrease_amount).unwrap();
    }

    require!(
        ctx.accounts.put_option_taker_info.qty_deposited <= ctx.accounts.put_option_taker_info.max_base_asset,
        PutOptionError::IllegalState
    );
    
    Ok(final_funding)
}

//remember, oracle should have written last fair price at most MAX_SECONDS_FROM_LAST_FAIR_PRICE_UPDATE before
pub fn taker_buy_lots_put_option_vault<'info>(ctx: Context<'_, '_, '_, 'info, TakerBuyLotsPutOptionVault<'info>>,
    max_fair_price: u64,
    num_lots_to_buy: u64,
    initial_funding: u64
) -> Result<TakerBuyLotsPutOptionReturn> {

    // Must pass PutOptionMakerInfo and corresponding quote asset ATAs (to receive premium) of potential sellers
    // in remaining accounts
    require!(
        ctx.remaining_accounts.len() > 0,
        PutOptionError::EmptyRemainingAccounts
    );


    // Always in pairs, first the PutOptionMakerInfo, followed by the seller ATA
    require!(
        ctx.remaining_accounts.len() % 2 == 0,
        PutOptionError::RemainingAccountsNumIsOdd
    );      


    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    // Period to take options is already closed
    require!(
        ctx.accounts.vault_factory_info.maturity > current_time.checked_add(FREEZE_SECONDS).unwrap(),
        PutOptionError::MaturityTooEarly
    );

    // If taker is entering the vault, we initialize her PutOptionTakerInfo
    // If she already has a PutOptionTakerInfo, she is buying more put options
    if !ctx.accounts.put_option_taker_info.is_initialized {
        require!(
            !ctx.accounts.vault_info.is_takers_full,
            PutOptionError::TakersFull
        );

        ctx.accounts.vault_info.takers_num = ctx.accounts.vault_info.takers_num.checked_add(1).unwrap();
        if ctx.accounts.vault_info.takers_num >= ctx.accounts.vault_info.max_takers {
            ctx.accounts.vault_info.is_takers_full = true;
        }

        ctx.accounts.put_option_taker_info.ord = ctx.accounts.vault_info.takers_num;
        ctx.accounts.put_option_taker_info.max_base_asset = 0;
        ctx.accounts.put_option_taker_info.qty_deposited = 0;
        ctx.accounts.put_option_taker_info.is_settled = false;
        ctx.accounts.put_option_taker_info.owner = ctx.accounts.initializer.key();
        ctx.accounts.put_option_taker_info.put_option_vault = ctx.accounts.vault_info.key();

        ctx.accounts.put_option_taker_info.is_initialized = true;
    }

    // We cannot have a timestamp for the last fair price in the future
    require!(
        ctx.accounts.vault_factory_info.ts_last_fair_price <= current_time,
        PutOptionError::IllegalState
    );

    // We only sell if the option price has been updated recently
    let seconds_from_update = current_time.checked_sub(ctx.accounts.vault_factory_info.ts_last_fair_price).unwrap();
    require!(
        seconds_from_update <= MAX_SECONDS_FROM_LAST_FAIR_PRICE_UPDATE,
        PutOptionError::LastFairPriceUpdateTooOld
    );

    // We won't sell if the taker is not willing to pay the current fair price
    require!(
        max_fair_price >= ctx.accounts.vault_factory_info.last_fair_price,
        PutOptionError::MaxFairPriceTooLow
    );

    let lot_multiplier:f64 = 10.0f64.powf(ctx.accounts.vault_info.lot_size as f64);
    require!(
        lot_multiplier.is_finite(),
        PutOptionError::Overflow
    );

    // How much does one lot costs in quote asset lamports
    let lot_price_in_quote_lamports_f64 = lot_multiplier*(ctx.accounts.vault_factory_info.strike as f64);
    require!(
        lot_price_in_quote_lamports_f64.is_finite(),
        PutOptionError::Overflow
    );
    require!(
        lot_price_in_quote_lamports_f64 > 0.0,
        PutOptionError::IllegalState
    );

    // Always use integer prices
    let lot_price_in_quote_lamports = lot_price_in_quote_lamports_f64.ceil() as u64;

    let mut total_lots_bought:u64 = 0;
    for i in 0..(ctx.remaining_accounts.len()/2) {
        let mut maker_info: Account<PutOptionMakerInfo> = PutOptionMakerInfo::from(&ctx.remaining_accounts[2*i]);
        let maker_ata:Account<TokenAccount> = Account::try_from(&ctx.remaining_accounts[2*i + 1])?;

        require!(
            maker_info.owner == maker_ata.owner,
            PutOptionError::AccountValidationError
        );

        require!(
            maker_info.put_option_vault == ctx.accounts.vault_info.key(),
            PutOptionError::AccountValidationError
        );

        require!(
            maker_ata.mint == ctx.accounts.quote_asset_mint.key(),
            PutOptionError::AccountValidationError
        );
        
        let maker_avbl_quote_asset = maker_info.quote_asset_qty.checked_sub(maker_info.volume_sold).unwrap();
        let max_lots_from_this_maker = maker_avbl_quote_asset.checked_div(lot_price_in_quote_lamports).unwrap();
        let lots_from_this_maker = std::cmp::min(max_lots_from_this_maker, num_lots_to_buy.checked_sub(total_lots_bought).unwrap());
        if lots_from_this_maker > 0 {
            let reserve_amount = lots_from_this_maker.checked_mul(lot_price_in_quote_lamports).unwrap();
            maker_info.volume_sold = maker_info.volume_sold.checked_add(reserve_amount).unwrap();
            ctx.accounts.vault_info.makers_total_pending_sell = ctx.accounts.vault_info.makers_total_pending_sell.checked_sub(reserve_amount).unwrap();
            let new_avbl_quote_asset = maker_info.quote_asset_qty.checked_sub(maker_info.volume_sold).unwrap();
            let new_avbl_lots = new_avbl_quote_asset.checked_div(lot_price_in_quote_lamports).unwrap();
            if new_avbl_lots < 1 {
                maker_info.is_all_sold = true;
            }
            // Now transfer the premium to the maker and protocol
            let premium_to_maker_f64 = (ctx.accounts.vault_factory_info.last_fair_price as f64)*lot_multiplier*(lots_from_this_maker as f64);
            require!(
                premium_to_maker_f64.is_finite() && premium_to_maker_f64 > 0.0,
                PutOptionError::IllegalState
            );
            let mut premium_to_maker = premium_to_maker_f64.round() as u64;
            let total_fees = premium_to_maker_f64*PROTOCOL_TOTAL_FEES;
            let backend_share = (total_fees*(1.0 - FRONTEND_SHARE)).ceil() as u64;
            let frontend_share = (total_fees*(FRONTEND_SHARE)).ceil() as u64;
            require!(
                premium_to_maker > backend_share + frontend_share,
                PutOptionError::OptionPremiumTooLow
            );
            premium_to_maker = premium_to_maker.checked_sub(backend_share).unwrap();
            premium_to_maker = premium_to_maker.checked_sub(frontend_share).unwrap();



            {
                let cpi_program = ctx.accounts.token_program.to_account_info();
                msg!("Started transferring premium lamports in quote asset from taker to maker");                
                let cpi_accounts = Transfer {
                    from: ctx.accounts.taker_quote_asset_account.to_account_info(),
                    to: maker_ata.to_account_info(),
                    authority: ctx.accounts.initializer.to_account_info(),
                };
                let token_transfer_context = CpiContext::new(cpi_program, cpi_accounts);
                token::transfer(token_transfer_context, premium_to_maker)?;
                msg!("Finished transferring premium quote asset lamports to maker");
            }

            {
                let cpi_program = ctx.accounts.token_program.to_account_info();
                msg!("Started transferring backend fee lamports to protocol");
                let cpi_accounts = Transfer {
                    from: ctx.accounts.taker_quote_asset_account.to_account_info(),
                    to: ctx.accounts.protocol_quote_asset_treasury.to_account_info(),
                    authority: ctx.accounts.initializer.to_account_info(),
                };
                let token_transfer_context = CpiContext::new(cpi_program, cpi_accounts);
                token::transfer(token_transfer_context, backend_share)?;
                msg!("Finished transferring backend fee lamports to protocol");
            }

            {
                let cpi_program = ctx.accounts.token_program.to_account_info();
                msg!("Started transferring frontend fee lamports to protocol");
                let cpi_accounts = Transfer {
                    from: ctx.accounts.taker_quote_asset_account.to_account_info(),
                    to: ctx.accounts.frontend_quote_asset_treasury.to_account_info(),
                    authority: ctx.accounts.initializer.to_account_info(),
                };
                let token_transfer_context = CpiContext::new(cpi_program, cpi_accounts);
                token::transfer(token_transfer_context, frontend_share)?;
                msg!("Finished transferring frontend fee lamports to protocol");

            }
        
            total_lots_bought = total_lots_bought.checked_add(lots_from_this_maker).unwrap();
            { // Serializing maker info
                let mut data = ctx.remaining_accounts[2*i].try_borrow_mut_data()?;
                maker_info.try_serialize(&mut data.as_mut())?;    
            }
            if total_lots_bought >= num_lots_to_buy {
                break;
            }
        }
    }
    require!(
        total_lots_bought <= num_lots_to_buy,
        PutOptionError::IllegalState
    );

    let mut base_asset_transfer_qty:u64 = 0;
    if total_lots_bought > 0 {
        let max_initial_funding_base_lamports_f64 = (total_lots_bought as f64)*lot_multiplier*(10.0f64.powf(ctx.accounts.base_asset_mint.decimals as f64));
        require!(
            max_initial_funding_base_lamports_f64.is_finite(),
            PutOptionError::Overflow
        );  
        let max_initial_funding_base_lamports = max_initial_funding_base_lamports_f64.ceil() as u64;
        ctx.accounts.put_option_taker_info.max_base_asset = ctx.accounts.put_option_taker_info.max_base_asset.checked_add(max_initial_funding_base_lamports).unwrap();
        if initial_funding > 0 {
            let missing_funding = ctx.accounts.put_option_taker_info.max_base_asset.checked_sub(ctx.accounts.put_option_taker_info.qty_deposited).unwrap();
            base_asset_transfer_qty = std::cmp::min(initial_funding, missing_funding);
            
            {
                let cpi_program = ctx.accounts.token_program.to_account_info();
                msg!("Started transferring base assets to fund option");
                let cpi_accounts = Transfer {
                    from: ctx.accounts.taker_base_asset_account.to_account_info(),
                    to: ctx.accounts.vault_base_asset_treasury.to_account_info(),
                    authority: ctx.accounts.initializer.to_account_info(),
                };
                let token_transfer_context = CpiContext::new(cpi_program, cpi_accounts);
                token::transfer(token_transfer_context, base_asset_transfer_qty)?;
                msg!("Finished transferring base assets to fund option")
            }            
            ctx.accounts.vault_info.takers_total_deposited = ctx.accounts.vault_info.takers_total_deposited.checked_add(base_asset_transfer_qty).unwrap();
            ctx.accounts.put_option_taker_info.qty_deposited = ctx.accounts.put_option_taker_info.qty_deposited.checked_add(base_asset_transfer_qty).unwrap();
        }    
    }

    require!(
        ctx.accounts.put_option_taker_info.qty_deposited <= ctx.accounts.put_option_taker_info.max_base_asset,
        PutOptionError::IllegalState
    );


    let result = TakerBuyLotsPutOptionReturn {
        num_lots_bought: total_lots_bought,
        price: ctx.accounts.vault_factory_info.last_fair_price,
        funding_added: base_asset_transfer_qty
    };
    Ok(result)
}

pub fn maker_adjust_position_put_option_vault(ctx: Context<MakerAdjustPositionPutOptionVault>,     
    num_lots_to_sell: u64,
    premium_limit: u64
) -> Result<()> {

    msg!("Entered maker_adjust_position_put_option_vault");
    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        ctx.accounts.vault_factory_info.maturity > current_time.checked_add(FREEZE_SECONDS).unwrap(),
        PutOptionError::MaturityTooEarly
    );

    require!(
        ctx.accounts.put_option_maker_info.is_settled == false,
        PutOptionError::IllegalState
    );

    let lot_multiplier:f64 = 10.0f64.powf(ctx.accounts.vault_info.lot_size as f64);
    let lot_value = lot_multiplier*(ctx.accounts.vault_factory_info.strike as f64);
    let rounded_lot_value = lot_value.ceil() as u64;

    let wanted_amount_f64 = (num_lots_to_sell as f64)*lot_value;
    require!(
        wanted_amount_f64.is_finite(),
        PutOptionError::Overflow
    );
    require!(
        wanted_amount_f64 >= 0.0,
        PutOptionError::IllegalState
    );
    
    let wanted_amount = wanted_amount_f64.ceil() as u64;

    if wanted_amount > ctx.accounts.put_option_maker_info.quote_asset_qty {
        // Maker wants to increase her position in the vault

        let increase_amount = wanted_amount.checked_sub(ctx.accounts.put_option_maker_info.quote_asset_qty).unwrap();
        // Proceed to transfer 
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = Transfer {
            from: ctx.accounts.maker_quote_asset_account.to_account_info(),
            to: ctx.accounts.vault_quote_asset_treasury.to_account_info(),
            authority: ctx.accounts.initializer.to_account_info(),
        };
        let token_transfer_context = CpiContext::new(cpi_program, cpi_accounts);

        token::transfer(token_transfer_context, increase_amount)?;
        msg!("Transferred {} USDC lamports to quote asset treasury", increase_amount);
        ctx.accounts.put_option_maker_info.quote_asset_qty = ctx.accounts.put_option_maker_info.quote_asset_qty.checked_add(increase_amount).unwrap();
        require!(
            ctx.accounts.put_option_maker_info.quote_asset_qty.checked_sub(ctx.accounts.put_option_maker_info.volume_sold).unwrap() >= rounded_lot_value,
            PutOptionError::IllegalState
        );
        ctx.accounts.put_option_maker_info.is_all_sold = false;
        ctx.accounts.vault_info.makers_total_pending_sell = ctx.accounts.vault_info.makers_total_pending_sell.checked_add(increase_amount).unwrap();
        ctx.accounts.vault_info.makers_total_pending_settle = ctx.accounts.vault_info.makers_total_pending_settle.checked_add(increase_amount).unwrap();

    } else if wanted_amount < ctx.accounts.put_option_maker_info.quote_asset_qty {
        // Maker wants to decrease her position in the vault
        let decrease_amount = ctx.accounts.put_option_maker_info.quote_asset_qty - wanted_amount;
        let max_decrease = ctx.accounts.put_option_maker_info.quote_asset_qty - ctx.accounts.put_option_maker_info.volume_sold;
        require!(
            decrease_amount <= max_decrease,
            PutOptionError::OversizedDecrease
        );
        // Proceed to transfer 
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = Transfer {
            from: ctx.accounts.vault_quote_asset_treasury.to_account_info(),
            to: ctx.accounts.maker_quote_asset_account.to_account_info(),
            authority: ctx.accounts.vault_info.to_account_info(),
        };

        // Preparing PDA signer
        let auth_bump = *ctx.bumps.get("vault_info").unwrap();
        let seeds = &[
            "PutOptionVaultInfo".as_bytes().as_ref(), 
            &ctx.accounts.vault_factory_info.key().to_bytes(),
            &ctx.accounts.vault_info.ord.to_le_bytes(),
            &[auth_bump],
        ];
        let signer = &[&seeds[..]];


        let token_transfer_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

        token::transfer(token_transfer_context, decrease_amount)?;
        msg!("Transferred {} USDC lamports from quote asset treasury to user", decrease_amount);
        ctx.accounts.put_option_maker_info.quote_asset_qty = ctx.accounts.put_option_maker_info.quote_asset_qty.checked_sub(decrease_amount).unwrap();
        ctx.accounts.put_option_maker_info.is_all_sold = ctx.accounts.put_option_maker_info.quote_asset_qty.checked_sub(ctx.accounts.put_option_maker_info.volume_sold).unwrap() < rounded_lot_value;
        ctx.accounts.vault_info.makers_total_pending_sell = ctx.accounts.vault_info.makers_total_pending_sell.checked_sub(decrease_amount).unwrap();
        ctx.accounts.vault_info.makers_total_pending_settle = ctx.accounts.vault_info.makers_total_pending_settle.checked_sub(decrease_amount).unwrap();

    }

    ctx.accounts.put_option_maker_info.premium_limit = premium_limit;    

    require!(
        ctx.accounts.put_option_maker_info.quote_asset_qty >= ctx.accounts.put_option_maker_info.volume_sold,
        PutOptionError::IllegalState
    );


    Ok(())

}

pub fn maker_enter_put_option_vault(ctx: Context<MakerEnterPutOptionVault>,     
    num_lots_to_sell: u64,
    premium_limit: u64
) -> Result<()> {

    require!(
        num_lots_to_sell > 0,
        PutOptionError::LotsToSellZero
    );

    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        ctx.accounts.vault_factory_info.maturity > current_time.checked_add(FREEZE_SECONDS).unwrap(),
        PutOptionError::MaturityTooEarly
    );

    ctx.accounts.vault_info.makers_num = ctx.accounts.vault_info.makers_num.checked_add(1).unwrap();

    if ctx.accounts.vault_info.makers_num >= ctx.accounts.vault_info.max_makers {
        ctx.accounts.vault_info.is_makers_full = true;
    }

    // Proceed to transfer 
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_accounts = Transfer {
        from: ctx.accounts.maker_quote_asset_account.to_account_info(),
        to: ctx.accounts.vault_quote_asset_treasury.to_account_info(),
        authority: ctx.accounts.initializer.to_account_info(),
    };
    let token_transfer_context = CpiContext::new(cpi_program, cpi_accounts);

    let lot_multiplier:f64 = 10.0f64.powf(ctx.accounts.vault_info.lot_size as f64);
    msg!("num_lots_to_sell: {}", num_lots_to_sell);
    msg!("lot_multiplier: {}", lot_multiplier);
    msg!("strike: {}", ctx.accounts.vault_factory_info.strike);
    let lot_value = lot_multiplier*(ctx.accounts.vault_factory_info.strike as f64);
    let rounded_lot_value = lot_value.ceil() as u64;

    let transfer_amount_f64 = (num_lots_to_sell as f64)*lot_value;
    require!(
        transfer_amount_f64.is_finite(),
        PutOptionError::Overflow
    );
    require!(
        transfer_amount_f64 >= 0.0,
        PutOptionError::IllegalState
    );
    
    let transfer_amount = transfer_amount_f64.ceil() as u64;

    require!(
        ctx.accounts.maker_quote_asset_account.amount >= transfer_amount,
        PutOptionError::InsufficientFunds
    );


    token::transfer(token_transfer_context, transfer_amount)?;
    msg!("Transferred {} USDC lamports to quote asset treasury", transfer_amount);

    // Updating vault_info ...
    ctx.accounts.vault_info.makers_total_pending_sell = ctx.accounts.vault_info.makers_total_pending_sell.checked_add(transfer_amount).unwrap();
    ctx.accounts.vault_info.makers_total_pending_settle = ctx.accounts.vault_info.makers_total_pending_settle.checked_add(transfer_amount).unwrap();
    msg!("Finished initialization of PutOptionVaultInfo, now initializing PutOptionMakerInfo");

    // Now initializing info about this maker in the vault (PutOptionMakerInfo)
    ctx.accounts.put_option_maker_info.ord = ctx.accounts.vault_info.makers_num;
    ctx.accounts.put_option_maker_info.quote_asset_qty = transfer_amount;
    ctx.accounts.put_option_maker_info.volume_sold = 0;
    ctx.accounts.put_option_maker_info.is_all_sold = false;
    require!(
        ctx.accounts.put_option_maker_info.quote_asset_qty >= rounded_lot_value,
        PutOptionError::IllegalState
    );
    ctx.accounts.put_option_maker_info.is_settled = false;
    ctx.accounts.put_option_maker_info.premium_limit = premium_limit;
    ctx.accounts.put_option_maker_info.owner = ctx.accounts.maker_quote_asset_account.owner;
    ctx.accounts.put_option_maker_info.put_option_vault = ctx.accounts.vault_info.key();
    msg!("Vault initialization finished");


    Ok(())
}

pub fn maker_create_put_option_vault(ctx: Context<MakerCreatePutOptionVault>,
    params: MakerCreatePutOptionParams, vault_id: u64
) -> Result<()> {

    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        params.maturity > current_time.checked_add(FREEZE_SECONDS).unwrap(),
        PutOptionError::MaturityTooEarly
    );


    // Initializing this new vault (PutOptionVaultInfo)
    // and updating number of vaults in factory
    msg!("Started initialization of PutOptionVaultInfo");
    ctx.accounts.vault_info.factory_vault = ctx.accounts.vault_factory_info.key();
    ctx.accounts.vault_info.ord = vault_id;
    ctx.accounts.vault_info.max_makers = params.max_makers;
    ctx.accounts.vault_info.max_takers = params.max_takers;
    ctx.accounts.vault_info.lot_size = params.lot_size;

    // Proceed to transfer (still initializing vault)
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_accounts = Transfer {
        from: ctx.accounts.maker_quote_asset_account.to_account_info(),
        to: ctx.accounts.vault_quote_asset_treasury.to_account_info(),
        authority: ctx.accounts.initializer.to_account_info(),
    };
    let token_transfer_context = CpiContext::new(cpi_program, cpi_accounts);

    let lot_multiplier:f64 = 10.0f64.powf(params.lot_size as f64);
    let lot_value = lot_multiplier*(params.strike as f64);
    let rounded_lot_value = lot_value.ceil() as u64;


    let transfer_amount_f64 = (params.num_lots_to_sell as f64)*lot_value;
    msg!("params.lot_size: {}", params.lot_size);
    msg!("num_lots_to_sell: {}", params.num_lots_to_sell);
    msg!("lot_multiplier: {}", lot_multiplier);
    msg!("strike: {}", params.strike);

    msg!("Transfer amount is {}", transfer_amount_f64);
    require!(
        transfer_amount_f64.is_finite(),
        PutOptionError::Overflow
    );
    require!(
        transfer_amount_f64 >= 0.0,
        PutOptionError::IllegalState
    );
    
    let transfer_amount = transfer_amount_f64.ceil() as u64;

    require!(
        ctx.accounts.maker_quote_asset_account.amount >= transfer_amount,
        PutOptionError::InsufficientFunds
    );

    token::transfer(token_transfer_context, transfer_amount)?;
    msg!("Transferred {} USDC lamports to quote asset treasury", transfer_amount);

    // Continuing to initialize vault...
    ctx.accounts.vault_info.makers_num = 1;
    ctx.accounts.vault_info.makers_total_pending_sell = transfer_amount;
    ctx.accounts.vault_info.makers_total_pending_settle = transfer_amount;
    ctx.accounts.vault_info.is_makers_full = ctx.accounts.vault_info.makers_num >= ctx.accounts.vault_info.max_makers; 
    ctx.accounts.vault_info.takers_num = 0;
    ctx.accounts.vault_info.takers_total_deposited = 0;
    ctx.accounts.vault_info.is_takers_full = ctx.accounts.vault_info.takers_num >= ctx.accounts.vault_info.max_takers;
    ctx.accounts.vault_info.bonus_not_exercised = 0;
    msg!("Finished initialization of PutOptionVaultInfo, now initializing PutOptionMakerInfo");

    // Now initializing info about this maker in the vault (PutOptionMakerInfo)
    ctx.accounts.put_option_maker_info.ord = ctx.accounts.vault_info.makers_num;
    ctx.accounts.put_option_maker_info.quote_asset_qty = transfer_amount;
    ctx.accounts.put_option_maker_info.volume_sold = 0;
    ctx.accounts.put_option_maker_info.is_settled = false;
    ctx.accounts.put_option_maker_info.is_all_sold = false;
    require!(
        ctx.accounts.put_option_maker_info.quote_asset_qty >= rounded_lot_value,
        PutOptionError::IllegalState
    );
    ctx.accounts.put_option_maker_info.premium_limit = params.premium_limit;
    ctx.accounts.put_option_maker_info.owner = ctx.accounts.maker_quote_asset_account.owner;
    ctx.accounts.put_option_maker_info.put_option_vault = ctx.accounts.vault_info.key();
    msg!("Vault initialization finished");
    
    Ok(())
}
