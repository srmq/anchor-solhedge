use anchor_lang::prelude::*;

#[account]
pub struct PutOptionVaultFactoryInfo {
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

#[account]
pub struct PutOptionVaultInfo {
    pub factory_vault: Pubkey,

    pub ord: u64,
    pub max_makers: u16,
    pub max_takers: u16,
    pub lot_size: i8, //10^lot_size, for instance 0 means 1; -1 means 0.1; 2 means 100

    pub makers_num: u16,
    pub makers_total_pending_sell: u64,     // the amount of quote asset lamports that makers have deposited, but not yet sold
    pub makers_total_pending_settle: u64,   // the amount of quote asset lamports that has been sold in options and has not been settled
    pub is_makers_full: bool,

    pub takers_num: u16,
    pub takers_total_deposited: u64,        // the amount that takers have funded
    pub is_takers_full: bool,
    pub bonus_not_exercised: u64            // the amount of bonus that has been given to early settlers (makers) when the option was exercised
                                        // but the takers have not fully funded what they had bought.
}

#[account]
pub struct PutOptionMakerInfo {
    pub ord: u16,
    pub quote_asset_qty: u64,       // total in quote asset lamports that has been deposited by the maker
    pub volume_sold: u64,           // amount of quote asset lamports that has been sold in options by this maker (volume_sold <= quote_asset_qty)
    pub is_all_sold: bool,          // if the available volume (quote_asset_qty - volume_sold) is worth less than 1 lot, all is sold
    pub is_settled: bool,           // if the maker has already got his tokens after maturity
    pub premium_limit: u64,         // minimum price for option premium he is willing to get, can be zero if he is ok of selling at whatever the fair price
    pub owner: Pubkey,
    pub put_option_vault: Pubkey
}

impl PutOptionMakerInfo {
    pub fn from<'info>(info: &AccountInfo<'info>) -> Account<'info, Self> {
        Account::try_from(info).unwrap()
    }
}

#[account]
pub struct PutOptionTakerInfo {
    pub is_initialized: bool,

    pub ord: u16,
    pub max_base_asset: u64,        // the max amount in base asset lamports she may fund based on how many options she bought
    pub qty_deposited: u64,         // how much she has funded (qty_deposited <= max_base_asset)
    pub is_settled: bool,           // if the taker has already got her tokens after maturity
    pub owner: Pubkey,
    pub put_option_vault: Pubkey
}

#[account]
pub struct PutOptionUpdateFairPriceTicketInfo {
    pub is_used: bool,
    pub factory_vault: Pubkey
}

#[account]
pub struct PutOptionSettlePriceTicketInfo {
    pub is_used: bool,
    pub factory_vault: Pubkey
}
