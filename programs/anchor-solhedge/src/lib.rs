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
use anchor_lang::{prelude::*, solana_program, system_program};
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use anchor_spl::associated_token::AssociatedToken;
use solana_program::{pubkey, pubkey::Pubkey, sysvar::clock::Clock};


declare_id!("8DYMPBKLDULX6G7ZuNrs1FcjuMqJwefu2MEfxkCq4sWY");

//Options will be negotiated up to 30 minutes to maturity
const FREEZE_SECONDS: u64 = 30*60;

const LAMPORTS_FOR_UPDATE_FAIRPRICE_TICKET: u64 = 500000;
const LAMPORTS_FOR_UPDATE_SETTLEPRICE_TICKET: u64 = 500000;

const MAX_SECONDS_FROM_LAST_FAIR_PRICE_UPDATE: u64 = 60;

//At this moment we will create options for at most
//30 days in the future
const MAX_MATURITY_FUTURE_SECONDS: u64 = 30*24*60*60;

// The corresponding private key is public on oracle.ts and on github! MUST CHANGE ON REAL DEPLOYMENT!
const ORACLE_ADDRESS: Pubkey = pubkey!("9SBVhfXD73uNe9hQRLBBmzgY7PZUTQYGaa6aPM7Gqo68");

// The corresponding private key is public on anchor-solhedge.ts and on github! MUST CHANGE ON REAL DEPLOYMENT!
const PROTOCOL_FEES_ADDRESS: Pubkey = pubkey!("FGmbHBRXPe6gRUe9MzuRUVaCsnViUvvWpuyTD8sV8tuh");

const PROTOCOL_TOTAL_FEES:f64 = 0.01;
const FRONTEND_SHARE:f64 = 0.5;

#[program]
pub mod anchor_solhedge {
    use super::*;
    use anchor_spl::token::Transfer;

    #[derive(AnchorSerialize, AnchorDeserialize)]
    pub struct TakerBuyLotsPutOptionReturn {
        pub num_lots_bought: u64,
        pub price: u64,
        pub funding_added: u64
    }

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }

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

            ctx.accounts.vault_factory_info.is_initialized = true;
            msg!("PutOptionVaultFactoryInfo initialized");
        }
        let result = ctx.accounts.vault_factory_info.next_vault_id;
        ctx.accounts.vault_factory_info.next_vault_id = ctx.accounts.vault_factory_info.next_vault_id.checked_add(1).unwrap();

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

}

#[derive(Accounts)]
pub struct Initialize {}

#[derive(Accounts)]
#[instruction(
    new_funding: u64
)]
pub struct TakerAdjustFundingPutOptionVault<'info> {
    #[account(
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.matured == false,
        constraint = vault_factory_info.base_asset == base_asset_mint.key(),
        constraint = vault_factory_info.quote_asset == quote_asset_mint.key(),

    )]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    #[account(
        mut,
        seeds=[
            "PutOptionVaultInfo".as_bytes().as_ref(), 
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref()
        ], bump,
        constraint = vault_info.factory_vault == vault_factory_info.key(),
    )]
    pub vault_info: Account<'info, PutOptionVaultInfo>,

    #[account(
        mut,
        seeds=[
            "PutOptionTakerInfo".as_bytes().as_ref(),
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref(), 
            initializer.key().as_ref()
        ],
        bump,
        constraint = !put_option_taker_info.is_settled
    )]
    pub put_option_taker_info: Account<'info, PutOptionTakerInfo>,

    // mint for the base_asset
    pub base_asset_mint: Account<'info, Mint>,

    // mint for the quote asset
    pub quote_asset_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = vault_base_asset_treasury.mint == base_asset_mint.key(), // Base asset mint
        constraint = vault_base_asset_treasury.owner.key() == vault_info.key() // Authority set to vault PDA
    )]
    pub vault_base_asset_treasury: Box<Account<'info, TokenAccount>>,

    // deposit of funding will come/go from/to this account
    #[account(
        mut,
        constraint = taker_base_asset_account.owner.key() == initializer.key(),
        constraint = taker_base_asset_account.mint == base_asset_mint.key()
    )]
    pub taker_base_asset_account: Box<Account<'info, TokenAccount>>,

    // Check if initializer is signer, mut is required to reduce lamports (fees)
    #[account(mut)]
    pub initializer: Signer<'info>,
    
    // System Program requred for deduction of lamports (fees)
    pub system_program: Program<'info, System>,
    // Token Program required to call transfer instruction
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>
}

#[derive(Accounts)]
#[instruction(
    max_fair_price: u64,
    num_lots_to_buy: u64,
    initial_funding: u64
)]
pub struct TakerBuyLotsPutOptionVault<'info> {
    #[account(
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.matured == false,
        constraint = vault_factory_info.base_asset == base_asset_mint.key(),
        constraint = vault_factory_info.quote_asset == quote_asset_mint.key(),

    )]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    #[account(
        mut,
        seeds=[
            "PutOptionVaultInfo".as_bytes().as_ref(), 
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref()
        ], bump,
        constraint = vault_info.factory_vault == vault_factory_info.key(),
    )]
    pub vault_info: Account<'info, PutOptionVaultInfo>,

    #[account(
        init_if_needed,
        seeds=[
            "PutOptionTakerInfo".as_bytes().as_ref(),
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref(), 
            initializer.key().as_ref()
        ],
        bump,
        payer = initializer,
        space = std::mem::size_of::<PutOptionTakerInfo>() + 8,
        constraint = !put_option_taker_info.is_settled
    )]
    pub put_option_taker_info: Account<'info, PutOptionTakerInfo>,


    // mint for the base_asset
    pub base_asset_mint: Account<'info, Mint>,

    // mint for the quote asset
    pub quote_asset_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = vault_base_asset_treasury.mint == base_asset_mint.key(), // Base asset mint
        constraint = vault_base_asset_treasury.owner.key() == vault_info.key() // Authority set to vault PDA
    )]
    pub vault_base_asset_treasury: Box<Account<'info, TokenAccount>>,

    // to pay the option premium (fair price)
    #[account(
        mut,
        constraint = taker_quote_asset_account.owner.key() == initializer.key(),
        constraint = taker_quote_asset_account.mint == quote_asset_mint.key()
    )]
    pub taker_quote_asset_account: Box<Account<'info, TokenAccount>>,

    // deposit of initial funding will come from here
    #[account(
        mut,
        constraint = taker_base_asset_account.owner.key() == initializer.key(),
        constraint = taker_base_asset_account.mint == base_asset_mint.key()
    )]
    pub taker_base_asset_account: Box<Account<'info, TokenAccount>>,

    // protocol fees will be paid here
    #[account(
        mut,
        constraint = protocol_quote_asset_treasury.owner.key() == PROTOCOL_FEES_ADDRESS,
        constraint = protocol_quote_asset_treasury.mint == quote_asset_mint.key()
    )]
    pub protocol_quote_asset_treasury: Box<Account<'info, TokenAccount>>,

    // frontend fees will be paid here
    #[account(
        mut,
        constraint = frontend_quote_asset_treasury.mint == quote_asset_mint.key()
    )]
    pub frontend_quote_asset_treasury: Box<Account<'info, TokenAccount>>,

    // Check if initializer is signer, mut is required to reduce lamports (fees)
    #[account(mut)]
    pub initializer: Signer<'info>,
    
    // System Program requred for deduction of lamports (fees)
    pub system_program: Program<'info, System>,
    // Token Program required to call transfer instruction
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,

}

#[derive(Accounts)]
#[instruction(
    settle_price: u64
)]
pub struct OracleUpdateSettlePrice<'info> {
    #[account(
        mut,
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
    )]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    #[account(
        mut,
        seeds=["PutOptionSettlePriceTicketInfo".as_bytes().as_ref(), vault_factory_info.key().as_ref(), ticket_owner.key().as_ref()],
        bump,
        close = ticket_owner,
        constraint = update_ticket.is_used == false, 
    )]
    pub update_ticket: Account<'info, PutOptionSettlePriceTicketInfo>,

    #[account(
        mut
    )]
    pub ticket_owner: SystemAccount<'info>,

    // Check if initializer is signer, should also be the oracle, mut is required to reduce lamports (fees)
    #[account(
        mut,
        constraint = initializer.key() == ORACLE_ADDRESS
    )]
    pub initializer: Signer<'info>,

    // System Program requred for deduction of lamports (fees)
    pub system_program: Program<'info, System>

}


#[derive(Accounts)]
#[instruction(
    new_fair_price: u64
)]

pub struct OracleUpdateFairPrice<'info> {
    #[account(
        mut,
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
    )]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    #[account(
        mut,
        seeds=["PutOptionUpdateTicketInfo".as_bytes().as_ref(), vault_factory_info.key().as_ref(), ticket_owner.key().as_ref()],
        bump,
        close = ticket_owner,
        constraint = update_ticket.is_used == false, 
    )]
    pub update_ticket: Account<'info, PutOptionUpdateFairPriceTicketInfo>,

    #[account(
        mut
    )]
    pub ticket_owner: SystemAccount<'info>,

    // Check if initializer is signer, should also be the oracle, mut is required to reduce lamports (fees)
    #[account(
        mut,
        constraint = initializer.key() == ORACLE_ADDRESS
    )]
    pub initializer: Signer<'info>,

    // System Program requred for deduction of lamports (fees)
    pub system_program: Program<'info, System>

}

#[derive(Accounts)]
pub struct GenSettlePutOptionPriceTicket<'info> {
    #[account(
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.matured == false,
        constraint = vault_factory_info.is_initialized == true
    )]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    #[account(
        init,
        seeds=["PutOptionSettlePriceTicketInfo".as_bytes().as_ref(), vault_factory_info.key().as_ref(), initializer.key().as_ref()],
        bump,
        payer = initializer,
        space = std::mem::size_of::<PutOptionSettlePriceTicketInfo>() + 8,
    )]
    pub put_option_settle_price_ticket: Account<'info, PutOptionSettlePriceTicketInfo>,

    // Check if initializer is signer, mut is required to reduce lamports (fees)
    #[account(mut)]
    pub initializer: Signer<'info>,

    #[account(
        mut,
        constraint = oracle_wallet.key() == ORACLE_ADDRESS
    )]
    pub oracle_wallet: SystemAccount<'info>,

    // System Program requred for deduction of lamports (fees)
    pub system_program: Program<'info, System>

}

#[derive(Accounts)]
pub struct GenUpdatePutOptionFairPriceTicket<'info> {
    #[account(
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.matured == false,
        constraint = vault_factory_info.is_initialized == true,
    )]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    #[account(
        init,
        seeds=["PutOptionUpdateTicketInfo".as_bytes().as_ref(), vault_factory_info.key().as_ref(), initializer.key().as_ref()],
        bump,
        payer = initializer,
        space = std::mem::size_of::<PutOptionUpdateFairPriceTicketInfo>() + 8,
    )]
    pub put_option_fair_price_ticket: Account<'info, PutOptionUpdateFairPriceTicketInfo>,

    // Check if initializer is signer, mut is required to reduce lamports (fees)
    #[account(mut)]
    pub initializer: Signer<'info>,

    #[account(
        mut,
        constraint = oracle_wallet.key() == ORACLE_ADDRESS
    )]
    pub oracle_wallet: SystemAccount<'info>,

    // System Program requred for deduction of lamports (fees)
    pub system_program: Program<'info, System>
}

#[derive(Accounts)]
#[instruction(
    params: MakerCreatePutOptionParams
)]
pub struct MakerNextPutOptionVaultId<'info> {
    #[account(
        init_if_needed, 
        seeds=["PutOptionVaultFactoryInfo".as_bytes().as_ref(), base_asset_mint.key().as_ref(), quote_asset_mint.key().as_ref(), &params.maturity.to_le_bytes().as_ref(), &params.strike.to_le_bytes().as_ref()], 
        bump, 
        payer = initializer, 
        space= std::mem::size_of::<PutOptionVaultFactoryInfo>() + 8,        
        constraint = params.strike > 0
    )]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    // mint for the base_asset
    pub base_asset_mint: Account<'info, Mint>,

    // mint for the quote asset
    pub quote_asset_mint: Account<'info, Mint>,

    // Check if initializer is signer, mut is required to reduce lamports (fees)
    #[account(mut)]
    pub initializer: Signer<'info>,
    // System Program requred for deduction of lamports (fees)
    pub system_program: Program<'info, System>
}

#[derive(Accounts)]
#[instruction(
    num_lots_to_sell: u64,
    premium_limit: u64
)]
pub struct MakerAdjustPositionPutOptionVault<'info> {
    #[account(        
        seeds=["PutOptionVaultFactoryInfo".as_bytes().as_ref(), base_asset_mint.key().as_ref(), quote_asset_mint.key().as_ref(), vault_factory_info.maturity.to_le_bytes().as_ref(), vault_factory_info.strike.to_le_bytes().as_ref()], 
        bump, 
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.matured == false,
        constraint = vault_factory_info.base_asset == base_asset_mint.key(),
        constraint = vault_factory_info.quote_asset == quote_asset_mint.key(),

    )]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    #[account(
        mut,
        seeds=[
            "PutOptionVaultInfo".as_bytes().as_ref(), 
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref()
        ], bump,
        constraint = vault_info.factory_vault == vault_factory_info.key(),
    )]
    pub vault_info: Account<'info, PutOptionVaultInfo>,

    // mint for the base_asset
    pub base_asset_mint: Account<'info, Mint>,

    // mint for the quote asset
    pub quote_asset_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = vault_quote_asset_treasury.mint == quote_asset_mint.key(), // Quote asset mint
        constraint = vault_quote_asset_treasury.owner == vault_info.key() // Authority set to vault PDA
    )]
    pub vault_quote_asset_treasury: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds=[
            "PutOptionMakerInfo".as_bytes().as_ref(),
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref(), 
            initializer.key().as_ref()
        ],
        bump,
        constraint = put_option_maker_info.put_option_vault == vault_info.key(),
        constraint = put_option_maker_info.owner == initializer.key()
    )]
    pub put_option_maker_info: Account<'info, PutOptionMakerInfo>,

    #[account(
        mut,
        constraint = maker_quote_asset_account.owner.key() == initializer.key(),
        constraint = maker_quote_asset_account.mint == quote_asset_mint.key()
    )]
    pub maker_quote_asset_account: Box<Account<'info, TokenAccount>>,


    // Check if initializer is signer, mut is required to reduce lamports (fees)
    #[account(mut)]
    pub initializer: Signer<'info>,
    
    // System Program requred for deduction of lamports (fees)
    pub system_program: Program<'info, System>,
    // Token Program required to call transfer instruction
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,

}

#[derive(Accounts)]
#[instruction(
    num_lots_to_sell: u64,
    premium_limit: u64
)]
pub struct MakerEnterPutOptionVault<'info> {
    #[account(
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.matured == false,
        constraint = vault_factory_info.base_asset == base_asset_mint.key(),
        constraint = vault_factory_info.quote_asset == quote_asset_mint.key(),

    )]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    #[account(
        mut,
        seeds=[
            "PutOptionVaultInfo".as_bytes().as_ref(), 
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref()
        ], bump,
        constraint = vault_info.is_makers_full == false,
        constraint = vault_info.factory_vault == vault_factory_info.key(),
    )]
    pub vault_info: Account<'info, PutOptionVaultInfo>,

    // mint for the base_asset
    pub base_asset_mint: Account<'info, Mint>,

    // mint for the quote asset
    pub quote_asset_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = vault_quote_asset_treasury.mint == quote_asset_mint.key(), // Quote asset mint
        constraint = vault_quote_asset_treasury.owner.key() == vault_info.key() // Authority set to vault PDA
    )]
    pub vault_quote_asset_treasury: Box<Account<'info, TokenAccount>>,

    #[account(
        init,
        seeds=[
            "PutOptionMakerInfo".as_bytes().as_ref(),
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref(), 
            initializer.key().as_ref()
        ],
        bump,
        payer = initializer,
        space = std::mem::size_of::<PutOptionMakerInfo>() + 8
    )]
    pub put_option_maker_info: Account<'info, PutOptionMakerInfo>,

    #[account(
        mut,
        constraint = maker_quote_asset_account.owner.key() == initializer.key(),
        constraint = maker_quote_asset_account.mint == quote_asset_mint.key()
    )]
    pub maker_quote_asset_account: Box<Account<'info, TokenAccount>>,


    // Check if initializer is signer, mut is required to reduce lamports (fees)
    #[account(mut)]
    pub initializer: Signer<'info>,
    
    // System Program requred for deduction of lamports (fees)
    pub system_program: Program<'info, System>,
    // Token Program required to call transfer instruction
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,

}

#[derive(Accounts)]
#[instruction(
    params: MakerCreatePutOptionParams, 
    vault_id: u64
)]
pub struct MakerCreatePutOptionVault<'info> {
    #[account(
        seeds=["PutOptionVaultFactoryInfo".as_bytes().as_ref(), base_asset_mint.key().as_ref(), quote_asset_mint.key().as_ref(), &params.maturity.to_le_bytes().as_ref(), &params.strike.to_le_bytes().as_ref()], 
        bump, 
        constraint = params.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.base_asset == base_asset_mint.key(),
        constraint = vault_factory_info.quote_asset == quote_asset_mint.key(),
        constraint = vault_factory_info.maturity == params.maturity,
        constraint = vault_factory_info.strike == params.strike,
    )]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    #[account(
        init,
        seeds=[
            "PutOptionVaultInfo".as_bytes().as_ref(), 
            vault_factory_info.key().as_ref(),
            &vault_id.to_le_bytes().as_ref()
        ],
        bump,
        payer = initializer, 
        constraint = vault_id < vault_factory_info.next_vault_id,
        space= std::mem::size_of::<PutOptionVaultInfo>() + 8
    )]
    pub vault_info: Account<'info, PutOptionVaultInfo>,

    // mint for the base_asset
    pub base_asset_mint: Account<'info, Mint>,

    // mint for the quote asset
    pub quote_asset_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = initializer, // Payer will be initializer
        associated_token::mint = base_asset_mint, 
        associated_token::authority = vault_info // Authority set to PDA
    )]
    pub vault_base_asset_treasury: Box<Account<'info, TokenAccount>>,

    #[account(
        init,
        payer = initializer, // Payer will be initializer
        associated_token::mint = quote_asset_mint, // Quote asset mint
        associated_token::authority = vault_info // Authority set to vault PDA
    )]
    pub vault_quote_asset_treasury: Box<Account<'info, TokenAccount>>,

    #[account(
        init,
        seeds=[
            "PutOptionMakerInfo".as_bytes().as_ref(),
            vault_factory_info.key().as_ref(),
            &vault_id.to_le_bytes().as_ref(), 
            initializer.key().as_ref()
        ],
        bump,
        payer = initializer,
        space = std::mem::size_of::<PutOptionMakerInfo>() + 8
    )]
    pub put_option_maker_info: Account<'info, PutOptionMakerInfo>,

    #[account(
        mut,
        constraint = maker_quote_asset_account.owner.key() == initializer.key(),
        constraint = maker_quote_asset_account.mint == quote_asset_mint.key()
    )]
    pub maker_quote_asset_account: Box<Account<'info, TokenAccount>>,

    // Check if initializer is signer, mut is required to reduce lamports (fees)
    #[account(mut)]
    pub initializer: Signer<'info>,
    
    // System Program requred for deduction of lamports (fees)
    pub system_program: Program<'info, System>,
    // Token Program required to call transfer instruction
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[account]
pub struct PutOptionVaultFactoryInfo {
    is_initialized: bool,

    next_vault_id: u64,
    maturity: u64,
    matured: bool,
    strike: u64,
    base_asset: Pubkey,
    quote_asset: Pubkey,

    last_fair_price: u64,
    ts_last_fair_price: u64,
    settled_price: u64
}

#[account]
pub struct PutOptionVaultInfo {
    factory_vault: Pubkey,

    ord: u64,
    max_makers: u16,
    max_takers: u16,
    lot_size: i8, //10^lot_size, for instance 0 means 1; -1 means 0.1; 2 means 100

    makers_num: u16,
    makers_total_pending_sell: u64,     // the amount of quote asset lamports that makers have deposited, but not yet sold
    makers_total_pending_settle: u64,   // the amount of quote asset lamports that has been sold in options and has not been settled
    is_makers_full: bool,

    takers_num: u16,
    takers_total_deposited: u64,        // the amount that takers have funded
    is_takers_full: bool
}

#[account]
pub struct PutOptionMakerInfo {
    ord: u16,
    quote_asset_qty: u64,       // total in quote asset lamports that has been deposited by the maker
    volume_sold: u64,           // amount of quote asset lamports that has been sold in options by this maker (volume_sold <= quote_asset_qty)
    is_all_sold: bool,          // if the available volume (quote_asset_qty - volume_sold) is worth less than 1 lot, all is sold
    is_settled: bool,           // if the maker has already got his tokens after maturity
    premium_limit: u64,         // minimum price for option premium he is willing to get, can be zero if he is ok of selling at whatever the fair price
    owner: Pubkey,
    put_option_vault: Pubkey
}


impl PutOptionMakerInfo {
    fn from<'info>(info: &AccountInfo<'info>) -> Account<'info, Self> {
        Account::try_from(info).unwrap()
    }
}

#[account]
pub struct PutOptionSettlePriceTicketInfo {
    is_used: bool,
    factory_vault: Pubkey
}


#[account]
pub struct PutOptionUpdateFairPriceTicketInfo {
    is_used: bool,
    factory_vault: Pubkey
}

#[account]
pub struct PutOptionTakerInfo {
    is_initialized: bool,

    ord: u16,
    max_base_asset: u64,        // the max amount in base asset lamports she may fund based on how many options she bought
    qty_deposited: u64,         // how much she has funded (qty_deposited <= max_base_asset)
    is_settled: bool,           // if the taker has already got her tokens after maturity
    owner: Pubkey,
    put_option_vault: Pubkey
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct MakerCreatePutOptionParams {
    maturity: u64, 
    strike: u64,
    max_makers: u16,
    max_takers: u16,
    lot_size: i8, //10^lot_size, for instance 0 means 1; -1 means 0.1; 2 means 100
    num_lots_to_sell: u64,
    premium_limit: u64
}

#[error_code]
pub enum PutOptionError {
    #[msg("Number of max_makers cannot be zero")]
    MaxMakersZero,

    #[msg("Number of max_takers cannot be zero")]
    MaxTakersZero,

    #[msg("num_lots_to_sell cannot be zero")]
    LotsToSellZero,

    #[msg("strike cannot be zero")]
    StrikeZero,

    #[msg("Price cannot be zero")]
    PriceZero,

    #[msg("maturity is too early")]
    MaturityTooEarly,

    #[msg("maturity is too late")]
    MaturityTooLate,

    #[msg("Unable to decrease position given previous commitments")]
    OversizedDecrease,

    #[msg("Overflow error")]
    Overflow,

    #[msg("Illegal internal state")]
    IllegalState,

    #[msg("Update put option fair price ticket is already used")]
    UsedUpdateTicket,

    #[msg("Not enough funds in source account")]
    InsufficientFunds,

    #[msg("No more takers are allowed in this vault")]
    TakersFull,

    #[msg("Last fair price update is too old. Please ask the oracle to make a new update")]
    LastFairPriceUpdateTooOld,

    #[msg("Your max fair price is below current fair price")]
    MaxFairPriceTooLow,

    #[msg("Remaining accounts are empty")]
    EmptyRemainingAccounts,
  
    #[msg("Quantity of remaining accounts is odd, should be even")]
    RemainingAccountsNumIsOdd,

    #[msg("Account validation error")]
    AccountValidationError,

    #[msg("Option premium price is too low")]
    OptionPremiumTooLow


}    
