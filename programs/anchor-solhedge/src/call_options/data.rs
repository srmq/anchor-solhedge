use anchor_lang::prelude::*;

#[account]
pub struct CallOptionVaultFactoryInfo {
    pub is_initialized: bool,

    pub next_vault_id: u64,
    pub maturity: u64,
    pub matured: bool,
    pub strike: u64,
    pub base_asset: Pubkey,
    pub quote_asset: Pubkey,

    pub last_fair_price: u64,
    pub ts_last_fair_price: u64,
    pub settled_price: u64,
    pub emergency_mode: bool
}
