use anchor_lang::prelude::*;
use crate::call_options::validators::*;
use crate::MakerCreateCallOptionParams;
use crate::call_options::errors::CallOptionError;
use crate::{
    FREEZE_SECONDS, 
    MAX_MATURITY_FUTURE_SECONDS,
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
