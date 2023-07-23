/* 
    anchor-solhedge
    Copyright (C) 2023 Sergio Queiroz <srmq@srmq.org>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU Affero General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU Affero General Public License for more details.

    You should have received a copy of the GNU Affero General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */
use anchor_lang::{prelude::*, solana_program};
use solana_program::{pubkey, pubkey::Pubkey};

use put_options::validators::*;
use call_options::validators::*;
use put_options::po_controller as po;
use call_options::co_controller as co;


mod put_options;
mod call_options;

declare_id!("FoUvjSVZMDccmb2fCppM24N8yzVpPMKYn1h2CZDV7FFa");

//Options will be negotiated up to 30 minutes to maturity
pub const FREEZE_SECONDS: u64 = 30*60;

pub const LAMPORTS_FOR_UPDATE_FAIRPRICE_TICKET: u64 = 500000;
pub const LAMPORTS_FOR_UPDATE_SETTLEPRICE_TICKET: u64 = 500000;

pub const MAX_SECONDS_FROM_LAST_FAIR_PRICE_UPDATE: u64 = 60;

// At this moment we will create options for at most
// 30 days in the future
pub const MAX_MATURITY_FUTURE_SECONDS: u64 = 30*24*60*60;

// If 15 days have passed after vault factory matured
// and the oracle has not updated settled price, we
// may assume that the whole world outside the blockchain
// has collapsed and we will let makers and takers take
// back their deposited assets, like if the option
// has not been exercised
pub const EMERGENCY_MODE_GRACE_PERIOD: u64 = 15*24*60*60;

// The corresponding private key should be on .env as DEVNET_ORACLE_KEY
pub const ORACLE_ADDRESS: Pubkey = pubkey!("Fr9SMCeLe7GoLUQq6URuvZUgaCWtEkmD7d18H6DSn81t");

// The corresponding private key should be on .env as DEVNET_PROTOCOL_FEES_KEY
pub const PROTOCOL_FEES_ADDRESS: Pubkey = pubkey!("Dku3bu5hqZVBXR39s6UW65nTPQ9rjevhhrfKhfpqgi8D");

pub const PROTOCOL_TOTAL_FEES:f64 = 0.01;
pub const FRONTEND_SHARE:f64 = 0.5;

#[program]
pub mod anchor_solhedge {
    use super::*;

    #[derive(AnchorSerialize, AnchorDeserialize)]
    pub struct TakerBuyLotsPutOptionReturn {
        pub num_lots_bought: u64,
        pub price: u64,
        pub funding_added: u64
    }

    #[derive(AnchorSerialize, AnchorDeserialize)]
    pub struct PutOptionSettleReturn {
        pub settle_result: PutOptionSettleResult,
        pub base_asset_transfer: u64,
        pub quote_asset_transfer: u64
    }
    

    #[derive(AnchorSerialize, AnchorDeserialize, Clone)]
    pub enum PutOptionSettleResult {
        NotExercised,
        FullyExercised,
        PartiallyExercised
    }

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }

    //----------- START PUT OPTIONS FAÇADE ------------------------------/
    pub fn oracle_update_put_option_settle_price(
        ctx: Context<OracleUpdatePutOptionSettlePrice>,
        settle_price: u64
    ) -> Result<()> {
        po::oracle_update_put_option_settle_price(ctx, settle_price)
    }

    pub fn oracle_update_put_option_price(
        ctx: Context<OracleUpdatePutOptionFairPrice>,
        new_fair_price: u64
    ) -> Result<()> {
        po::oracle_update_put_option_price(ctx, new_fair_price)
    }

    pub fn gen_settle_put_option_price_ticket(ctx: Context<GenSettlePutOptionPriceTicket>) -> Result<()> {
        po::gen_settle_put_option_price_ticket(ctx)
    }

    pub fn gen_update_put_option_fair_price_ticket(ctx: Context<GenUpdatePutOptionFairPriceTicket>) -> Result<()> {
        po::gen_update_put_option_fair_price_ticket(ctx)
    }

    pub fn maker_next_put_option_vault_id(ctx: Context<MakerNextPutOptionVaultId>,
        params: MakerCreatePutOptionParams
    ) -> Result<u64> {
        po::maker_next_put_option_vault_id(ctx, params)
    }

    pub fn maker_activate_put_option_emergency_mode(ctx: Context<MakerActivatePutOptionEmergencyMode>) -> Result<()> {
        po::maker_activate_put_option_emergency_mode(ctx)
    }

    pub fn taker_put_option_emergency_exit(ctx: Context<TakerPutOptionEmergencyExit>) -> Result<()> {
        po::taker_put_option_emergency_exit(ctx)
    }    

    pub fn maker_put_option_emergency_exit(ctx: Context<MakerPutOptionEmergencyExit>) -> Result<()> {
        po::maker_put_option_emergency_exit(ctx)
    }

    pub fn taker_activate_put_option_emergency_mode(ctx: Context<TakerActivatePutOptionEmergencyMode>) -> Result<()> {
        po::taker_activate_put_option_emergency_mode(ctx)
    }


    pub fn maker_settle_put_option(ctx: Context<MakerSettlePutOption>) -> Result<PutOptionSettleReturn> {
        po::maker_settle_put_option(ctx)
    }

    pub fn taker_settle_put_option(ctx: Context<TakerSettlePutOption>) -> Result<PutOptionSettleReturn> {
        po::taker_settle_put_option(ctx)
    }

    pub fn taker_adjust_funding_put_option_vault(ctx: Context<TakerAdjustFundingPutOptionVault>,
        new_funding: u64
    ) -> Result<u64> {
        po::taker_adjust_funding_put_option_vault(ctx, new_funding)
    }

    //remember, oracle should have written last fair price at most MAX_SECONDS_FROM_LAST_FAIR_PRICE_UPDATE before
    pub fn taker_buy_lots_put_option_vault<'info>(ctx: Context<'_, '_, '_, 'info, TakerBuyLotsPutOptionVault<'info>>,
        max_fair_price: u64,
        num_lots_to_buy: u64,
        initial_funding: u64
    ) -> Result<TakerBuyLotsPutOptionReturn> {
        po::taker_buy_lots_put_option_vault(ctx, max_fair_price, num_lots_to_buy, initial_funding)
    }

    pub fn maker_adjust_position_put_option_vault(ctx: Context<MakerAdjustPositionPutOptionVault>,     
        num_lots_to_sell: u64,
        premium_limit: u64
    ) -> Result<()> {

        po::maker_adjust_position_put_option_vault(ctx, num_lots_to_sell, premium_limit)
    }

    pub fn maker_enter_put_option_vault(ctx: Context<MakerEnterPutOptionVault>,     
        num_lots_to_sell: u64,
        premium_limit: u64
    ) -> Result<()> {

        po::maker_enter_put_option_vault(ctx, num_lots_to_sell, premium_limit)        
    }

    pub fn maker_create_put_option_vault(ctx: Context<MakerCreatePutOptionVault>,
        params: MakerCreatePutOptionParams, vault_id: u64
    ) -> Result<()> {
        po::maker_create_put_option_vault(ctx, params, vault_id)
    }
    //----------- END PUT OPTIONS FAÇADE ------------------------------/

    //----------- START CALL OPTIONS FAÇADE ------------------------------/
    pub fn maker_next_call_option_vault_id(ctx: Context<MakerNextCallOptionVaultId>,
        params: MakerCreateCallOptionParams
    ) -> Result<u64> {
        co::maker_next_call_option_vault_id(ctx, params)
    }

    pub fn maker_create_call_option_vault(ctx: Context<MakerCreateCallOptionVault>,
        params: MakerCreateCallOptionParams, vault_id: u64
    ) -> Result<()> {
        co::maker_create_call_option_vault(ctx, params, vault_id)
    }

    pub fn maker_enter_call_option_vault(ctx: Context<MakerEnterCallOptionVault>,     
        num_lots_to_sell: u64,
        premium_limit: u64
    ) -> Result<()> {
        co::maker_enter_call_option_vault(ctx, num_lots_to_sell, premium_limit)
    }

    pub fn maker_adjust_position_call_option_vault(ctx: Context<MakerAdjustPositionCallOptionVault>,     
        num_lots_to_sell: u64,
        premium_limit: u64
    ) -> Result<()> {
        co::maker_adjust_position_call_option_vault(ctx, num_lots_to_sell, premium_limit)
    }
    //----------- END CALL OPTIONS FAÇADE ------------------------------/

}


#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct MakerCreatePutOptionParams {
    pub maturity: u64, 
    pub strike: u64,
    pub max_makers: u16,
    pub max_takers: u16,
    pub lot_size: i8, //10^lot_size, for instance 0 means 1; -1 means 0.1; 2 means 100
    pub num_lots_to_sell: u64,
    pub premium_limit: u64
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct MakerCreateCallOptionParams {
    pub maturity: u64, 
    pub strike: u64,
    pub max_makers: u16,
    pub max_takers: u16,
    pub lot_size: i8, //10^lot_size, for instance 0 means 1; -1 means 0.1; 2 means 100
    pub num_lots_to_sell: u64,
    pub premium_limit: u64
}