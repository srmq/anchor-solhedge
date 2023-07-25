use anchor_lang::prelude::*;
use crate::MakerCreateCallOptionParams;
use crate::call_options::data::{
    CallOptionVaultFactoryInfo,
    CallOptionVaultInfo,
    CallOptionMakerInfo,
    CallOptionUpdateFairPriceTicketInfo
};
use anchor_spl::token::{Mint, Token, TokenAccount};
use anchor_spl::associated_token::AssociatedToken;
use crate::ORACLE_ADDRESS;

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

#[derive(Accounts)]
#[instruction(
    params: MakerCreateCallOptionParams, 
    vault_id: u64
)]
pub struct MakerCreateCallOptionVault<'info> {
    #[account(
        seeds=["CallOptionVaultFactoryInfo".as_bytes().as_ref(), base_asset_mint.key().as_ref(), quote_asset_mint.key().as_ref(), &params.maturity.to_le_bytes().as_ref(), &params.strike.to_le_bytes().as_ref()], 
        bump, 
        constraint = params.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.base_asset == base_asset_mint.key(),
        constraint = vault_factory_info.quote_asset == quote_asset_mint.key(),
        constraint = vault_factory_info.maturity == params.maturity,
        constraint = vault_factory_info.strike == params.strike,
        constraint = vault_factory_info.emergency_mode == false
    )]
    pub vault_factory_info: Account<'info, CallOptionVaultFactoryInfo>,

    #[account(
        init,
        seeds=[
            "CallOptionVaultInfo".as_bytes().as_ref(), 
            vault_factory_info.key().as_ref(),
            &vault_id.to_le_bytes().as_ref()
        ],
        bump,
        payer = initializer, 
        constraint = vault_id < vault_factory_info.next_vault_id,
        space= std::mem::size_of::<CallOptionVaultInfo>() + 8
    )]
    pub vault_info: Account<'info, CallOptionVaultInfo>,

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
            "CallOptionMakerInfo".as_bytes().as_ref(),
            vault_factory_info.key().as_ref(),
            &vault_id.to_le_bytes().as_ref(), 
            initializer.key().as_ref()
        ],
        bump,
        payer = initializer,
        space = std::mem::size_of::<CallOptionMakerInfo>() + 8
    )]
    pub call_option_maker_info: Account<'info, CallOptionMakerInfo>,

    #[account(
        mut,
        constraint = maker_base_asset_account.owner.key() == initializer.key(),
        constraint = maker_base_asset_account.mint == base_asset_mint.key()
    )]
    pub maker_base_asset_account: Box<Account<'info, TokenAccount>>,

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
pub struct MakerEnterCallOptionVault<'info> {
    #[account(
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.matured == false,
        constraint = vault_factory_info.base_asset == base_asset_mint.key(),
        constraint = vault_factory_info.quote_asset == quote_asset_mint.key(),
        constraint = vault_factory_info.emergency_mode == false
    )]
    pub vault_factory_info: Account<'info, CallOptionVaultFactoryInfo>,

    #[account(
        mut,
        seeds=[
            "CallOptionVaultInfo".as_bytes().as_ref(), 
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref()
        ], bump,
        constraint = vault_info.is_makers_full == false,
        constraint = vault_info.factory_vault == vault_factory_info.key(),
    )]
    pub vault_info: Account<'info, CallOptionVaultInfo>,

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

    #[account(
        init,
        seeds=[
            "CallOptionMakerInfo".as_bytes().as_ref(),
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref(), 
            initializer.key().as_ref()
        ],
        bump,
        payer = initializer,
        space = std::mem::size_of::<CallOptionMakerInfo>() + 8
    )]
    pub call_option_maker_info: Account<'info, CallOptionMakerInfo>,

    #[account(
        mut,
        constraint = maker_base_asset_account.owner.key() == initializer.key(),
        constraint = maker_base_asset_account.mint == base_asset_mint.key()
    )]
    pub maker_base_asset_account: Box<Account<'info, TokenAccount>>,


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
pub struct MakerAdjustPositionCallOptionVault<'info> {
    #[account(        
        seeds=["CallOptionVaultFactoryInfo".as_bytes().as_ref(), base_asset_mint.key().as_ref(), quote_asset_mint.key().as_ref(), vault_factory_info.maturity.to_le_bytes().as_ref(), vault_factory_info.strike.to_le_bytes().as_ref()], 
        bump, 
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.matured == false,
        constraint = vault_factory_info.base_asset == base_asset_mint.key(),
        constraint = vault_factory_info.quote_asset == quote_asset_mint.key(),
        constraint = vault_factory_info.emergency_mode == false
    )]
    pub vault_factory_info: Account<'info, CallOptionVaultFactoryInfo>,

    #[account(
        mut,
        seeds=[
            "CallOptionVaultInfo".as_bytes().as_ref(), 
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref()
        ], bump,
        constraint = vault_info.factory_vault == vault_factory_info.key(),
    )]
    pub vault_info: Account<'info, CallOptionVaultInfo>,

    // mint for the base_asset
    pub base_asset_mint: Account<'info, Mint>,

    // mint for the quote asset
    pub quote_asset_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = vault_base_asset_treasury.mint == base_asset_mint.key(), // Base asset mint
        constraint = vault_base_asset_treasury.owner == vault_info.key() // Authority set to vault PDA
    )]
    pub vault_base_asset_treasury: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds=[
            "CallOptionMakerInfo".as_bytes().as_ref(),
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref(), 
            initializer.key().as_ref()
        ],
        bump,
        constraint = call_option_maker_info.call_option_vault == vault_info.key(),
        constraint = call_option_maker_info.owner == initializer.key()
    )]
    pub call_option_maker_info: Account<'info, CallOptionMakerInfo>,

    #[account(
        mut,
        constraint = maker_base_asset_account.owner.key() == initializer.key(),
        constraint = maker_base_asset_account.mint == base_asset_mint.key()
    )]
    pub maker_base_asset_account: Box<Account<'info, TokenAccount>>,


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
pub struct GenUpdateCallOptionFairPriceTicket<'info> {
    #[account(
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.matured == false,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.emergency_mode == false
    )]
    pub vault_factory_info: Account<'info, CallOptionVaultFactoryInfo>,

    #[account(
        init,
        seeds=["CallOptionUpdateTicketInfo".as_bytes().as_ref(), vault_factory_info.key().as_ref(), initializer.key().as_ref()],
        bump,
        payer = initializer,
        space = std::mem::size_of::<CallOptionUpdateFairPriceTicketInfo>() + 8,
    )]
    pub call_option_fair_price_ticket: Account<'info, CallOptionUpdateFairPriceTicketInfo>,

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
    new_fair_price: u64
)]
pub struct OracleUpdateCallOptionFairPrice<'info> {
    #[account(
        mut,
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.emergency_mode == false
    )]
    pub vault_factory_info: Account<'info, CallOptionVaultFactoryInfo>,

    #[account(
        mut,
        seeds=["CallOptionUpdateTicketInfo".as_bytes().as_ref(), vault_factory_info.key().as_ref(), ticket_owner.key().as_ref()],
        bump,
        close = ticket_owner,
        constraint = update_ticket.is_used == false, 
    )]
    pub update_ticket: Account<'info, CallOptionUpdateFairPriceTicketInfo>,

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
