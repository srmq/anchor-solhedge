use anchor_lang::{prelude::*, system_program};
use crate::call_options::validators::*;
use crate::MakerCreateCallOptionParams;
use crate::call_options::errors::CallOptionError;
use crate::{
    FREEZE_SECONDS, 
    MAX_MATURITY_FUTURE_SECONDS,
    LAMPORTS_FOR_UPDATE_FAIRPRICE_TICKET,
    MAX_SECONDS_FROM_LAST_FAIR_PRICE_UPDATE,
    PROTOCOL_TOTAL_FEES,
    FRONTEND_SHARE
};
use anchor_spl::token::{self, Transfer, TokenAccount};
use crate::anchor_solhedge::*;
use crate::call_options::data::CallOptionMakerInfo;

pub fn maker_next_call_option_vault_id(ctx: Context<MakerNextCallOptionVaultId>,
    params: MakerCreateCallOptionParams
) -> Result<u64> {

    require!(
        params.strike > 0,
        CallOptionError::StrikeZero
    );

    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        params.maturity > current_time.checked_add(FREEZE_SECONDS).unwrap(),
        CallOptionError::MaturityTooEarly
    );

    require!(
        params.maturity <= current_time.checked_add(MAX_MATURITY_FUTURE_SECONDS).unwrap(),
        CallOptionError::MaturityTooLate
    );


    // Initializing factory vault (CallOptionVaultFactoryInfo) if it has been just created
    if !ctx.accounts.vault_factory_info.is_initialized {
        ctx.accounts.vault_factory_info.next_vault_id = 1;
        ctx.accounts.vault_factory_info.maturity = params.maturity;
        ctx.accounts.vault_factory_info.matured = false;
        ctx.accounts.vault_factory_info.strike = params.strike;
        ctx.accounts.vault_factory_info.base_asset = ctx.accounts.base_asset_mint.key();
        ctx.accounts.vault_factory_info.quote_asset = ctx.accounts.quote_asset_mint.key();
        ctx.accounts.vault_factory_info.emergency_mode = false;

        ctx.accounts.vault_factory_info.is_initialized = true;
        msg!("CallOptionVaultFactoryInfo initialized");
    }
    let result = ctx.accounts.vault_factory_info.next_vault_id;
    ctx.accounts.vault_factory_info.next_vault_id = ctx.accounts.vault_factory_info.next_vault_id.checked_add(1).unwrap();

    Ok(result)
}

pub fn maker_create_call_option_vault(ctx: Context<MakerCreateCallOptionVault>,
    params: MakerCreateCallOptionParams, vault_id: u64
) -> Result<()> {

    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        params.maturity > current_time.checked_add(FREEZE_SECONDS).unwrap(),
        CallOptionError::MaturityTooEarly
    );


    // Initializing this new vault (CallOptionVaultInfo)
    // and updating number of vaults in factory
    msg!("Started initialization of CallOptionVaultInfo");
    ctx.accounts.vault_info.factory_vault = ctx.accounts.vault_factory_info.key();
    ctx.accounts.vault_info.ord = vault_id;
    ctx.accounts.vault_info.max_makers = params.max_makers;
    ctx.accounts.vault_info.max_takers = params.max_takers;
    ctx.accounts.vault_info.lot_size = params.lot_size;

    // Proceed to transfer (still initializing vault)
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_accounts = Transfer {
        from: ctx.accounts.maker_base_asset_account.to_account_info(),
        to: ctx.accounts.vault_base_asset_treasury.to_account_info(),
        authority: ctx.accounts.initializer.to_account_info(),
    };
    let token_transfer_context = CpiContext::new(cpi_program, cpi_accounts);

    let lot_multiplier:f64 = 10.0f64.powf(params.lot_size as f64);
    let lot_lamports_qty = lot_multiplier*10.0f64.powf(ctx.accounts.base_asset_mint.decimals as f64);
    let rounded_lamports_qty = lot_lamports_qty.ceil() as u64;


    let transfer_amount_f64 = (params.num_lots_to_sell as f64)*(rounded_lamports_qty as f64);
    msg!("params.lot_size: {}", params.lot_size);
    msg!("num_lots_to_sell: {}", params.num_lots_to_sell);
    msg!("lot_multiplier: {}", lot_multiplier);
    msg!("strike: {}", params.strike);

    msg!("Transfer amount is {}", transfer_amount_f64);
    require!(
        transfer_amount_f64.is_finite(),
        CallOptionError::Overflow
    );
    require!(
        transfer_amount_f64 >= 0.0,
        CallOptionError::IllegalState
    );
    
    let transfer_amount = transfer_amount_f64.ceil() as u64;

    require!(
        ctx.accounts.maker_base_asset_account.amount >= transfer_amount,
        CallOptionError::InsufficientFunds
    );

    token::transfer(token_transfer_context, transfer_amount)?;
    msg!("Transferred {} base asset lamports to base asset treasury", transfer_amount);

    // Continuing to initialize vault...
    ctx.accounts.vault_info.makers_num = 1;
    ctx.accounts.vault_info.makers_total_pending_sell = transfer_amount;
    ctx.accounts.vault_info.makers_total_pending_settle = transfer_amount;
    ctx.accounts.vault_info.is_makers_full = ctx.accounts.vault_info.makers_num >= ctx.accounts.vault_info.max_makers; 
    ctx.accounts.vault_info.takers_num = 0;
    ctx.accounts.vault_info.takers_total_deposited = 0;
    ctx.accounts.vault_info.is_takers_full = ctx.accounts.vault_info.takers_num >= ctx.accounts.vault_info.max_takers;
    ctx.accounts.vault_info.bonus_not_exercised = 0;
    msg!("Finished initialization of CallOptionVaultInfo, now initializing CallOptionMakerInfo");

    // Now initializing info about this maker in the vault (CallOptionMakerInfo)
    ctx.accounts.call_option_maker_info.ord = ctx.accounts.vault_info.makers_num;
    ctx.accounts.call_option_maker_info.base_asset_qty = transfer_amount;
    ctx.accounts.call_option_maker_info.volume_sold = 0;
    ctx.accounts.call_option_maker_info.is_settled = false;
    ctx.accounts.call_option_maker_info.is_all_sold = false;
    require!(
        ctx.accounts.call_option_maker_info.base_asset_qty >= rounded_lamports_qty,
        CallOptionError::IllegalState
    );
    ctx.accounts.call_option_maker_info.premium_limit = params.premium_limit;
    ctx.accounts.call_option_maker_info.owner = ctx.accounts.maker_base_asset_account.owner;
    ctx.accounts.call_option_maker_info.call_option_vault = ctx.accounts.vault_info.key();
    msg!("Vault initialization finished");
    
    Ok(())
}

pub fn maker_enter_call_option_vault(ctx: Context<MakerEnterCallOptionVault>,     
    num_lots_to_sell: u64,
    premium_limit: u64
) -> Result<()> {

    require!(
        num_lots_to_sell > 0,
        CallOptionError::LotsToSellZero
    );

    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        ctx.accounts.vault_factory_info.maturity > current_time.checked_add(FREEZE_SECONDS).unwrap(),
        CallOptionError::MaturityTooEarly
    );

    ctx.accounts.vault_info.makers_num = ctx.accounts.vault_info.makers_num.checked_add(1).unwrap();

    if ctx.accounts.vault_info.makers_num >= ctx.accounts.vault_info.max_makers {
        ctx.accounts.vault_info.is_makers_full = true;
    }

    // Proceed to transfer 
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_accounts = Transfer {
        from: ctx.accounts.maker_base_asset_account.to_account_info(),
        to: ctx.accounts.vault_base_asset_treasury.to_account_info(),
        authority: ctx.accounts.initializer.to_account_info(),
    };
    let token_transfer_context = CpiContext::new(cpi_program, cpi_accounts);

    let lot_multiplier:f64 = 10.0f64.powf(ctx.accounts.vault_info.lot_size as f64);
    let lot_lamports_qty = lot_multiplier*10.0f64.powf(ctx.accounts.base_asset_mint.decimals as f64);
    let rounded_lamports_qty = lot_lamports_qty.ceil() as u64;


    let transfer_amount_f64 = (num_lots_to_sell as f64)*(rounded_lamports_qty as f64);
    msg!("params.lot_size: {}", ctx.accounts.vault_info.lot_size);
    msg!("num_lots_to_sell: {}", num_lots_to_sell);
    msg!("strike: {}", ctx.accounts.vault_factory_info.strike);
    msg!("lamports rounded qty for 1 lot: {}", rounded_lamports_qty);

    msg!("Transfer amount is {}", transfer_amount_f64);
    require!(
        transfer_amount_f64.is_finite(),
        CallOptionError::Overflow
    );
    require!(
        transfer_amount_f64 >= 0.0,
        CallOptionError::IllegalState
    );
    
    let transfer_amount = transfer_amount_f64.ceil() as u64;

    require!(
        ctx.accounts.maker_base_asset_account.amount >= transfer_amount,
        CallOptionError::InsufficientFunds
    );

    token::transfer(token_transfer_context, transfer_amount)?;
    msg!("Transferred {} base asset lamports to base asset treasury", transfer_amount);

    // Updating vault_info ...
    ctx.accounts.vault_info.makers_total_pending_sell = ctx.accounts.vault_info.makers_total_pending_sell.checked_add(transfer_amount).unwrap();
    ctx.accounts.vault_info.makers_total_pending_settle = ctx.accounts.vault_info.makers_total_pending_settle.checked_add(transfer_amount).unwrap();
    msg!("Finished transferring base assets to vault base asset treasury, now updating CallOptionMakerInfo");

    // Now initializing info about this maker in the vault (CallOptionMakerInfo)
    ctx.accounts.call_option_maker_info.ord = ctx.accounts.vault_info.makers_num;
    ctx.accounts.call_option_maker_info.base_asset_qty = transfer_amount;
    ctx.accounts.call_option_maker_info.volume_sold = 0;
    ctx.accounts.call_option_maker_info.is_all_sold = false;
    require!(
        ctx.accounts.call_option_maker_info.base_asset_qty >= rounded_lamports_qty,
        CallOptionError::IllegalState
    );
    ctx.accounts.call_option_maker_info.is_settled = false;
    ctx.accounts.call_option_maker_info.premium_limit = premium_limit;
    ctx.accounts.call_option_maker_info.owner = ctx.accounts.maker_base_asset_account.owner;
    ctx.accounts.call_option_maker_info.call_option_vault = ctx.accounts.vault_info.key();
    msg!("Vault initialization finished");


    Ok(())
}

pub fn maker_adjust_position_call_option_vault(ctx: Context<MakerAdjustPositionCallOptionVault>,     
    num_lots_to_sell: u64,
    premium_limit: u64
) -> Result<()> {

    msg!("Entered maker_adjust_position_call_option_vault");
    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        ctx.accounts.vault_factory_info.maturity > current_time.checked_add(FREEZE_SECONDS).unwrap(),
        CallOptionError::MaturityTooEarly
    );

    require!(
        ctx.accounts.call_option_maker_info.is_settled == false,
        CallOptionError::IllegalState
    );

    let lot_multiplier:f64 = 10.0f64.powf(ctx.accounts.vault_info.lot_size as f64);
    let lot_lamports_qty = lot_multiplier*10.0f64.powf(ctx.accounts.base_asset_mint.decimals as f64);
    let rounded_lamports_qty = lot_lamports_qty.ceil() as u64;


    let wanted_amount = rounded_lamports_qty.checked_mul(num_lots_to_sell).unwrap();


    if wanted_amount > ctx.accounts.call_option_maker_info.base_asset_qty {
        // Maker wants to increase her position in the vault

        let increase_amount = wanted_amount.checked_sub(ctx.accounts.call_option_maker_info.base_asset_qty).unwrap();
        // Proceed to transfer 
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = Transfer {
            from: ctx.accounts.maker_base_asset_account.to_account_info(),
            to: ctx.accounts.vault_base_asset_treasury.to_account_info(),
            authority: ctx.accounts.initializer.to_account_info(),
        };
        let token_transfer_context = CpiContext::new(cpi_program, cpi_accounts);

        token::transfer(token_transfer_context, increase_amount)?;
        msg!("Transferred {} base asset lamports to base asset treasury", increase_amount);
        ctx.accounts.call_option_maker_info.base_asset_qty = ctx.accounts.call_option_maker_info.base_asset_qty.checked_add(increase_amount).unwrap();
        require!(
            ctx.accounts.call_option_maker_info.base_asset_qty.checked_sub(ctx.accounts.call_option_maker_info.volume_sold).unwrap() >= rounded_lamports_qty,
            CallOptionError::IllegalState
        );
        ctx.accounts.call_option_maker_info.is_all_sold = false;
        ctx.accounts.vault_info.makers_total_pending_sell = ctx.accounts.vault_info.makers_total_pending_sell.checked_add(increase_amount).unwrap();
        ctx.accounts.vault_info.makers_total_pending_settle = ctx.accounts.vault_info.makers_total_pending_settle.checked_add(increase_amount).unwrap();

    } else if wanted_amount < ctx.accounts.call_option_maker_info.base_asset_qty {
        // Maker wants to decrease her position in the vault
        let decrease_amount = ctx.accounts.call_option_maker_info.base_asset_qty - wanted_amount;
        let max_decrease = ctx.accounts.call_option_maker_info.base_asset_qty - ctx.accounts.call_option_maker_info.volume_sold;
        require!(
            decrease_amount <= max_decrease,
            CallOptionError::OversizedDecrease
        );
        // Proceed to transfer 
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = Transfer {
            from: ctx.accounts.vault_base_asset_treasury.to_account_info(),
            to: ctx.accounts.maker_base_asset_account.to_account_info(),
            authority: ctx.accounts.vault_info.to_account_info(),
        };

        // Preparing PDA signer
        let auth_bump = *ctx.bumps.get("vault_info").unwrap();
        let seeds = &[
            "CallOptionVaultInfo".as_bytes().as_ref(), 
            &ctx.accounts.vault_factory_info.key().to_bytes(),
            &ctx.accounts.vault_info.ord.to_le_bytes(),
            &[auth_bump],
        ];
        let signer = &[&seeds[..]];


        let token_transfer_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

        token::transfer(token_transfer_context, decrease_amount)?;
        msg!("Transferred {} base asset lamports from base asset treasury to user", decrease_amount);
        ctx.accounts.call_option_maker_info.base_asset_qty = ctx.accounts.call_option_maker_info.base_asset_qty.checked_sub(decrease_amount).unwrap();
        ctx.accounts.call_option_maker_info.is_all_sold = ctx.accounts.call_option_maker_info.base_asset_qty.checked_sub(ctx.accounts.call_option_maker_info.volume_sold).unwrap() < rounded_lamports_qty;
        ctx.accounts.vault_info.makers_total_pending_sell = ctx.accounts.vault_info.makers_total_pending_sell.checked_sub(decrease_amount).unwrap();
        ctx.accounts.vault_info.makers_total_pending_settle = ctx.accounts.vault_info.makers_total_pending_settle.checked_sub(decrease_amount).unwrap();
    }

    ctx.accounts.call_option_maker_info.premium_limit = premium_limit;    

    require!(
        ctx.accounts.call_option_maker_info.base_asset_qty >= ctx.accounts.call_option_maker_info.volume_sold,
        CallOptionError::IllegalState
    );


    Ok(())

}

pub fn gen_update_call_option_fair_price_ticket(ctx: Context<GenUpdateCallOptionFairPriceTicket>) -> Result<()> {
    require!(
        ctx.accounts.call_option_fair_price_ticket.is_used == false,
        CallOptionError::UsedUpdateTicket
    );

    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    require!(
        ctx.accounts.vault_factory_info.maturity > current_time.checked_add(FREEZE_SECONDS).unwrap(),
        CallOptionError::MaturityTooEarly
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

pub fn oracle_update_call_option_price(
    ctx: Context<OracleUpdateCallOptionFairPrice>,
    new_fair_price: u64
) -> Result<()> {
    require!(
        new_fair_price > 0,
        CallOptionError::PriceZero
    );

    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    if ctx.accounts.vault_factory_info.maturity > current_time.checked_add(FREEZE_SECONDS).unwrap() {
        ctx.accounts.vault_factory_info.last_fair_price = new_fair_price;
        ctx.accounts.vault_factory_info.ts_last_fair_price = current_time;
    }
    ctx.accounts.update_ticket.is_used = true;
    Ok(())
}

//remember, oracle should have written last fair price at most MAX_SECONDS_FROM_LAST_FAIR_PRICE_UPDATE before
pub fn taker_buy_lots_call_option_vault<'info>(ctx: Context<'_, '_, '_, 'info, TakerBuyLotsCallOptionVault<'info>>,
    max_fair_price: u64,
    num_lots_to_buy: u64,
    initial_funding: u64
) -> Result<TakerBuyLotsCallOptionReturn> {

    // Must pass CallOptionMakerInfo and corresponding quote asset ATAs (to receive premium) of potential sellers
    // in remaining accounts
    require!(
        ctx.remaining_accounts.len() > 0,
        CallOptionError::EmptyRemainingAccounts
    );


    // Always in pairs, first the CallOptionMakerInfo, followed by the seller ATA
    require!(
        ctx.remaining_accounts.len() % 2 == 0,
        CallOptionError::RemainingAccountsNumIsOdd
    );      


    let current_time = Clock::get().unwrap().unix_timestamp as u64;
    // Period to take options is already closed
    require!(
        ctx.accounts.vault_factory_info.maturity > current_time.checked_add(FREEZE_SECONDS).unwrap(),
        CallOptionError::MaturityTooEarly
    );

    // If taker is entering the vault, we initialize her CallOptionTakerInfo
    // If she already has a CallOptionTakerInfo, she is buying more put options
    if !ctx.accounts.call_option_taker_info.is_initialized {
        require!(
            !ctx.accounts.vault_info.is_takers_full,
            CallOptionError::TakersFull
        );

        ctx.accounts.vault_info.takers_num = ctx.accounts.vault_info.takers_num.checked_add(1).unwrap();
        if ctx.accounts.vault_info.takers_num >= ctx.accounts.vault_info.max_takers {
            ctx.accounts.vault_info.is_takers_full = true;
        }

        ctx.accounts.call_option_taker_info.ord = ctx.accounts.vault_info.takers_num;
        ctx.accounts.call_option_taker_info.max_quote_asset = 0;
        ctx.accounts.call_option_taker_info.qty_deposited = 0;
        ctx.accounts.call_option_taker_info.is_settled = false;
        ctx.accounts.call_option_taker_info.owner = ctx.accounts.initializer.key();
        ctx.accounts.call_option_taker_info.call_option_vault = ctx.accounts.vault_info.key();

        ctx.accounts.call_option_taker_info.is_initialized = true;
    }

    // We cannot have a timestamp for the last fair price in the future
    require!(
        ctx.accounts.vault_factory_info.ts_last_fair_price <= current_time,
        CallOptionError::IllegalState
    );

    // We only sell if the option price has been updated recently
    let seconds_from_update = current_time.checked_sub(ctx.accounts.vault_factory_info.ts_last_fair_price).unwrap();
    require!(
        seconds_from_update <= MAX_SECONDS_FROM_LAST_FAIR_PRICE_UPDATE,
        CallOptionError::LastFairPriceUpdateTooOld
    );

    // We won't sell if the taker is not willing to pay the current fair price
    require!(
        max_fair_price >= ctx.accounts.vault_factory_info.last_fair_price,
        CallOptionError::MaxFairPriceTooLow
    );

    let lot_multiplier:f64 = 10.0f64.powf(ctx.accounts.vault_info.lot_size as f64);
    require!(
        lot_multiplier.is_finite(),
        CallOptionError::Overflow
    );

    // How much does one lot costs in quote asset lamports
    let lot_price_in_quote_lamports_f64 = lot_multiplier*(ctx.accounts.vault_factory_info.strike as f64);
    require!(
        lot_price_in_quote_lamports_f64.is_finite(),
        CallOptionError::Overflow
    );
    require!(
        lot_price_in_quote_lamports_f64 > 0.0,
        CallOptionError::IllegalState
    );

    // Always use integer prices
    let lot_price_in_quote_lamports = lot_price_in_quote_lamports_f64.ceil() as u64;

    let lot_in_base_lamports_f64 = lot_multiplier*(10.0f64.powf(ctx.accounts.base_asset_mint.decimals as f64));
    require!(
        lot_in_base_lamports_f64.is_finite() && lot_in_base_lamports_f64 > 0.0,
        CallOptionError::IllegalState
    );


    let lot_in_base_lamports = lot_in_base_lamports_f64.ceil() as u64;

    let mut total_lots_bought:u64 = 0;
    for i in 0..(ctx.remaining_accounts.len()/2) {
        let mut maker_info: Account<CallOptionMakerInfo> = CallOptionMakerInfo::from(&ctx.remaining_accounts[2*i]);
        let maker_ata:Account<TokenAccount> = Account::try_from(&ctx.remaining_accounts[2*i + 1])?;

        require!(
            maker_info.owner == maker_ata.owner,
            CallOptionError::AccountValidationError
        );

        require!(
            maker_info.call_option_vault == ctx.accounts.vault_info.key(),
            CallOptionError::AccountValidationError
        );

        require!(
            maker_ata.mint == ctx.accounts.quote_asset_mint.key(),
            CallOptionError::AccountValidationError
        );
        
        let maker_avbl_base_asset = maker_info.base_asset_qty.checked_sub(maker_info.volume_sold).unwrap();
        
        let max_lots_from_this_maker = maker_avbl_base_asset.checked_div(lot_in_base_lamports).unwrap();
        let lots_from_this_maker = std::cmp::min(max_lots_from_this_maker, num_lots_to_buy.checked_sub(total_lots_bought).unwrap());
        if lots_from_this_maker > 0 {
            let reserve_amount = lots_from_this_maker.checked_mul(lot_in_base_lamports).unwrap();
            maker_info.volume_sold = maker_info.volume_sold.checked_add(reserve_amount).unwrap();
            ctx.accounts.vault_info.makers_total_pending_sell = ctx.accounts.vault_info.makers_total_pending_sell.checked_sub(reserve_amount).unwrap();
            let new_avbl_base_asset = maker_info.base_asset_qty.checked_sub(maker_info.volume_sold).unwrap();
            let new_avbl_lots = new_avbl_base_asset.checked_div(lot_in_base_lamports).unwrap();
            if new_avbl_lots < 1 {
                maker_info.is_all_sold = true;
            }
            // Now transfer the premium to the maker and protocol
            let premium_to_maker_f64 = (ctx.accounts.vault_factory_info.last_fair_price as f64)*lot_multiplier*(lots_from_this_maker as f64);
            require!(
                premium_to_maker_f64.is_finite() && premium_to_maker_f64 > 0.0,
                CallOptionError::IllegalState
            );
            let mut premium_to_maker = premium_to_maker_f64.round() as u64;
            let total_fees = premium_to_maker_f64*PROTOCOL_TOTAL_FEES;
            let backend_share = (total_fees*(1.0 - FRONTEND_SHARE)).ceil() as u64;
            let frontend_share = (total_fees*(FRONTEND_SHARE)).ceil() as u64;
            require!(
                premium_to_maker > backend_share + frontend_share,
                CallOptionError::OptionPremiumTooLow
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
        CallOptionError::IllegalState
    );

    let mut quote_asset_transfer_qty:u64 = 0;
    if total_lots_bought > 0 {
        let max_initial_funding_quote_lamports = total_lots_bought.checked_mul(lot_price_in_quote_lamports).unwrap();
        ctx.accounts.call_option_taker_info.max_quote_asset = ctx.accounts.call_option_taker_info.max_quote_asset.checked_add(max_initial_funding_quote_lamports).unwrap();
        if initial_funding > 0 {
            let missing_funding = ctx.accounts.call_option_taker_info.max_quote_asset.checked_sub(ctx.accounts.call_option_taker_info.qty_deposited).unwrap();
            quote_asset_transfer_qty = std::cmp::min(initial_funding, missing_funding);
            
            {
                let cpi_program = ctx.accounts.token_program.to_account_info();
                msg!("Started transferring quote assets to fund option");
                let cpi_accounts = Transfer {
                    from: ctx.accounts.taker_quote_asset_account.to_account_info(),
                    to: ctx.accounts.vault_quote_asset_treasury.to_account_info(),
                    authority: ctx.accounts.initializer.to_account_info(),
                };
                let token_transfer_context = CpiContext::new(cpi_program, cpi_accounts);
                token::transfer(token_transfer_context, quote_asset_transfer_qty)?;
                msg!("Finished transferring quote assets to fund option")
            }            
            ctx.accounts.vault_info.takers_total_deposited = ctx.accounts.vault_info.takers_total_deposited.checked_add(quote_asset_transfer_qty).unwrap();
            ctx.accounts.call_option_taker_info.qty_deposited = ctx.accounts.call_option_taker_info.qty_deposited.checked_add(quote_asset_transfer_qty).unwrap();
        }    
    }

    require!(
        ctx.accounts.call_option_taker_info.qty_deposited <= ctx.accounts.call_option_taker_info.max_quote_asset,
        CallOptionError::IllegalState
    );


    let result = TakerBuyLotsCallOptionReturn {
        num_lots_bought: total_lots_bought,
        price: ctx.accounts.vault_factory_info.last_fair_price,
        funding_added: quote_asset_transfer_qty
    };
    Ok(result)
}
