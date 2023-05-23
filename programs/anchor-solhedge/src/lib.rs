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
use anchor_lang::{prelude::*};
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use anchor_spl::associated_token::AssociatedToken;

declare_id!("8DYMPBKLDULX6G7ZuNrs1FcjuMqJwefu2MEfxkCq4sWY");

const MAKER_COMISSION_PERCENT: f64 = 0.1;
const TAKER_COMISSION_PERCENT: f64 = 0.1;

#[program]
pub mod anchor_solhedge {
    use super::*;
    use anchor_spl::token::Transfer;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }

    pub fn maker_create_put_option_vault(ctx: Context<MakerCreatePutOptionVault>,
        maturity: u64, 
        strike: u64,
        max_makers: u16,
        max_takers: u16,
        lot_size: u64,
        min_ticker_increment: f32,
        num_lots_to_sell: u64,
        premium_limit: u64
    ) -> Result<()> {
        // Initializing factory vault (PutOptionVaultFactoryInfo) if it has been just created
        if !ctx.accounts.vault_factory_info.is_initialized {
            ctx.accounts.vault_factory_info.num_vaults = 0;
            ctx.accounts.vault_factory_info.maturity = maturity;
            ctx.accounts.vault_factory_info.strike = strike;
            ctx.accounts.vault_factory_info.base_asset = ctx.accounts.base_asset_mint.key();
            ctx.accounts.vault_factory_info.quote_asset = ctx.accounts.quote_asset_mint.key();

            ctx.accounts.vault_factory_info.is_initialized = true;
            msg!("PutOptionVaultFactoryInfo initialized");
        }

        // Initializing this new vault (PutOptionVaultInfo)
        // and updating number of vaults in factory
        msg!("Started initialization of PutOptionVaultInfo");
        ctx.accounts.vault_info.factory_vault = ctx.accounts.vault_factory_info.key();
        ctx.accounts.vault_factory_info.num_vaults += 1;
        ctx.accounts.vault_info.ord = ctx.accounts.vault_factory_info.num_vaults;
        ctx.accounts.vault_info.max_makers = max_makers;
        ctx.accounts.vault_info.max_takers = max_takers;
        ctx.accounts.vault_info.lot_size = lot_size;
        ctx.accounts.vault_info.min_ticker_increment = min_ticker_increment;

        // Proceed to transfer (still initializing vault)
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = Transfer {
            from: ctx.accounts.maker_quote_asset_account.to_account_info(),
            to: ctx.accounts.vault_quote_asset_treasury.to_account_info(),
            authority: ctx.accounts.initializer.to_account_info(),
        };
        let token_transfer_context = CpiContext::new(cpi_program, cpi_accounts);
        let transfer_amount = lot_size*num_lots_to_sell*strike;
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
        ctx.accounts.put_option_maker_info.premium_limit = premium_limit;
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
    maturity: u64, 
    strike: u64,
    max_makers: u16,
    max_takers: u16,
    lot_size: u64,
    min_ticker_increment: f32,
    num_lots_to_sell: u64,
    premium_limit: u64
)]
pub struct MakerCreatePutOptionVault<'info> {
    #[account(
        init_if_needed, 
        seeds=["PutOptionVaultFactoryInfo".as_bytes().as_ref(), base_asset_mint.key().as_ref(), quote_asset_mint.key().as_ref(), &maturity.to_le_bytes(), &strike.to_le_bytes()], 
        bump, 
        payer = initializer, 
        space= std::mem::size_of::<PutOptionVaultFactoryInfo>() + 8,
        constraint = strike > 0
    )]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    #[account(
        init,
        payer = initializer, 
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
        associated_token::authority = vault_factory_info // Authority set to PDA
    )]
    pub vault_base_asset_treasury: Box<Account<'info, TokenAccount>>,

    #[account(
        init,
        payer = initializer, // Payer will be initializer
        associated_token::mint = quote_asset_mint, // Quote asset mint
        associated_token::authority = vault_factory_info // Authority set to vault PDA
    )]
    pub vault_quote_asset_treasury: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = maker_quote_asset_account.owner.key() == initializer.key(),
        constraint = maker_quote_asset_account.mint == quote_asset_mint.key(),
        //constraint = maker_quote_asset_account.amount as f64 / 10.0f64.powi(quote_asset_mint.decimals as i32)  >= ((num_lots_to_sell as f64)*lot_size*strike)/(1.0-(MAKER_COMISSION_PERCENT/100.0))
    )]
    pub maker_quote_asset_account: Box<Account<'info, TokenAccount>>,

    #[account(
        init,
        payer = initializer,
        space = std::mem::size_of::<PutOptionMakerInfo>() + 8
    )]
    pub put_option_maker_info: Account<'info, PutOptionMakerInfo>,


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

    num_vaults: u64,
    maturity: u64,
    strike: u64,
    base_asset: Pubkey,
    quote_asset: Pubkey
}

#[account]
pub struct PutOptionVaultInfo {
    factory_vault: Pubkey,

    ord: u64,
    max_makers: u16,
    max_takers: u16,
    lot_size: u64,
    min_ticker_increment: f32,

    makers_num: u16,
    makers_total_pending_sell: u64,
    makers_total_pending_settle: u64,
    is_makers_full: bool,

    takers_num: u16,
    takers_total_deposited: u64,
    is_takers_full: bool
}

#[account]
pub struct PutOptionMakerInfo {
    ord: u16,
    quote_asset_qty: u64,
    volume_sold: u64,
    is_settled: bool,
    premium_limit: u64,
    owner: Pubkey,
    put_option_vault: Pubkey
}