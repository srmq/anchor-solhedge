use anchor_lang::prelude::*;
use crate::MakerCreateCallOptionParams;
use crate::call_options::data::CallOptionVaultFactoryInfo;
use anchor_spl::token::Mint;

#[derive(Accounts)]
#[instruction(
    params: MakerCreateCallOptionParams
)]
pub struct MakerNextCallOptionVaultId<'info> {
    #[account(
        init_if_needed, 
        seeds=["CallOptionVaultFactoryInfo".as_bytes().as_ref(), base_asset_mint.key().as_ref(), quote_asset_mint.key().as_ref(), &params.maturity.to_le_bytes().as_ref(), &params.strike.to_le_bytes().as_ref()], 
        bump, 
        payer = initializer, 
        space= std::mem::size_of::<CallOptionVaultFactoryInfo>() + 8,        
        constraint = params.strike > 0
    )]
    pub vault_factory_info: Account<'info, CallOptionVaultFactoryInfo>,

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
