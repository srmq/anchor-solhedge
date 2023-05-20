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
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use anchor_spl::associated_token::AssociatedToken;

declare_id!("8DYMPBKLDULX6G7ZuNrs1FcjuMqJwefu2MEfxkCq4sWY");

/// Seed for put option vault factory info account
const PUT_VAULT_FACTORY_SEED_PREFIX: &[u8] = b"putovfactinfo";
const USER_PUT_OPTION_VAULT_PARAMS : &[u8] = b"uputovparams";

#[program]
pub mod anchor_solhedge {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}

#[derive(Accounts)]
pub struct CreatePutOptionVault<'info> {
    #[account(init_if_needed, seeds=[PUT_VAULT_FACTORY_SEED_PREFIX, base_asset_mint.key().as_ref(), quote_asset_mint.key().as_ref(), &vault_params.maturity.to_le_bytes(), &vault_params.strike.to_le_bytes()], bump, payer = initializer, space= std::mem::size_of::<PutOptionVaultFactoryInfo>() + 8)]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    #[account(
        mut,
        seeds=[USER_PUT_OPTION_VAULT_PARAMS, initializer.key().as_ref(), base_asset_mint.key().as_ref(), quote_asset_mint.key().as_ref(), &vault_params.maturity.to_le_bytes(), &vault_params.strike.to_le_bytes()],
        bump,
        constraint = initializer.key() == vault_params.creator,
        constraint = vault_params.strike > 0.0,
        constraint = vault_params.min_order_size > 0.0,
        constraint = vault_params.min_ticker_increment > 0.0,
        close = initializer
    )]
    pub vault_params: Account<'info, UserPutOptionVaultParams>,

    // mint is required to create new account for PDA and for checking
    pub base_asset_mint: Account<'info, Mint>,

    // mint is required to create new account for PDA and for checking
    pub quote_asset_mint: Account<'info, Mint>,

    

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

    num_vaults: u32,
    maturity: u64,
    strike: f64,
    base_asset: Pubkey,
    quote_asset: Pubkey
}

#[account]
pub struct UserPutOptionVaultParams {
    creator: Pubkey,
    base_asset: Pubkey,
    quote_asset: Pubkey,
    maturity: u64,
    strike: f64,
    max_makers: u16,
    max_takers: u16,
    min_order_size: f64,
    min_ticker_increment: f32    
}

