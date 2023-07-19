use anchor_lang::prelude::*;
use crate::call_options::validators::*;
use crate::MakerCreateCallOptionParams;
use crate::call_options::errors::CallOptionError;
use crate::{
    FREEZE_SECONDS, 
    MAX_MATURITY_FUTURE_SECONDS,
};


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
