use anchor_lang::prelude::*;
use crate::put_options::validators::*;
use crate::put_options::errors::PutOptionError;
use crate::FREEZE_SECONDS;

pub fn oracle_update_settle_price(
    ctx: Context<OracleUpdateSettlePrice>,
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

pub fn oracle_update_price(
    ctx: Context<OracleUpdateFairPrice>,
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
