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

#[account]
pub struct CallOptionVaultInfo {
    pub factory_vault: Pubkey,

    pub ord: u64,
    pub max_makers: u16,
    pub max_takers: u16,
    pub lot_size: i8, //10^lot_size, for instance 0 means 1; -1 means 0.1; 2 means 100

    pub makers_num: u16,
    pub makers_total_pending_sell: u64,     // the amount of base asset lamports that makers have deposited, but not yet sold
    pub makers_total_pending_settle: u64,   // the amount of base asset lamports that has been sold in options and has not been settled
    pub is_makers_full: bool,

    pub takers_num: u16,
    pub takers_total_deposited: u64,        // the amount that takers have funded
    pub is_takers_full: bool,
    pub bonus_not_exercised: u64            // the amount of bonus that has been given to early settlers (makers) when the option was exercised
                                        // but the takers have not fully funded what they had bought.
}

#[account]
pub struct CallOptionMakerInfo {
    pub ord: u16,
    pub base_asset_qty: u64,       // total in base asset lamports that has been deposited by the maker
    pub volume_sold: u64,           // amount of base asset lamports that has been sold in options by this maker (volume_sold <= base_asset_qty)
    pub is_all_sold: bool,          // if the available volume (base_asset_qty - volume_sold) is worth less than 1 lot, all is sold
    pub is_settled: bool,           // if the maker has already got his tokens after maturity
    pub premium_limit: u64,         // minimum price for option premium he is willing to get, can be zero if he is ok of selling at whatever the fair price
    pub owner: Pubkey,
    pub call_option_vault: Pubkey
}

#[account]
pub struct CallOptionUpdateFairPriceTicketInfo {
    pub is_used: bool,
    pub factory_vault: Pubkey
}
