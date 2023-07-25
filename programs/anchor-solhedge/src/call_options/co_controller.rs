use anchor_lang::{prelude::*, system_program};
use crate::call_options::validators::*;
use crate::MakerCreateCallOptionParams;
use crate::call_options::errors::CallOptionError;
use crate::{
    FREEZE_SECONDS, 
    MAX_MATURITY_FUTURE_SECONDS,
    LAMPORTS_FOR_UPDATE_FAIRPRICE_TICKET,
};
use anchor_spl::token::{self, Transfer};

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
