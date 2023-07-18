use anchor_lang::prelude::*;
use crate::put_options::data::{
    PutOptionVaultFactoryInfo, 
    PutOptionVaultInfo, PutOptionMakerInfo, PutOptionTakerInfo,
    PutOptionUpdateFairPriceTicketInfo,
    PutOptionSettlePriceTicketInfo
};
use anchor_spl::token::{Mint, Token, TokenAccount};
use anchor_spl::associated_token::AssociatedToken;
use crate::{PROTOCOL_FEES_ADDRESS, ORACLE_ADDRESS};
use crate::MakerCreatePutOptionParams;

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
        constraint = vault_factory_info.emergency_mode == false
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
        constraint = vault_factory_info.emergency_mode == false
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
pub struct TakerPutOptionEmergencyExit<'info> {

    #[account(
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.matured == false,
        constraint = vault_factory_info.base_asset == base_asset_mint.key(),
        constraint = vault_factory_info.emergency_mode == true
    )]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    #[account(
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

    #[account(
        mut,
        constraint = vault_base_asset_treasury.mint == base_asset_mint.key(), // Base asset mint
        constraint = vault_base_asset_treasury.owner.key() == vault_info.key() // Authority set to vault PDA
    )]
    pub vault_base_asset_treasury: Box<Account<'info, TokenAccount>>,

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
pub struct MakerPutOptionEmergencyExit<'info> {
    #[account(
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.matured == false,
        constraint = vault_factory_info.quote_asset == quote_asset_mint.key(),
        constraint = vault_factory_info.emergency_mode == true
    )]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    #[account(
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
            "PutOptionMakerInfo".as_bytes().as_ref(),
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref(), 
            initializer.key().as_ref()
        ],
        bump,
        constraint = !put_option_maker_info.is_settled
    )]
    pub put_option_maker_info: Account<'info, PutOptionMakerInfo>,

    // mint for the quote asset
    pub quote_asset_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = vault_quote_asset_treasury.mint == quote_asset_mint.key(), // quote asset mint
        constraint = vault_quote_asset_treasury.owner.key() == vault_info.key() // Authority set to vault PDA
    )]
    pub vault_quote_asset_treasury: Box<Account<'info, TokenAccount>>,

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
    pub associated_token_program: Program<'info, AssociatedToken>
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
        constraint = vault_factory_info.emergency_mode == false
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


#[derive(Accounts)]
#[instruction(
    new_fair_price: u64
)]
pub struct OracleUpdatePutOptionFairPrice<'info> {
    #[account(
        mut,
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.emergency_mode == false
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
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.emergency_mode == false
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
        constraint = vault_factory_info.emergency_mode == false
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
    settle_price: u64
)]
pub struct OracleUpdatePutOptionSettlePrice<'info> {
    #[account(
        mut,
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.emergency_mode == false
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
pub struct TakerSettlePutOption<'info> {
    #[account(
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.matured == true,
        constraint = vault_factory_info.settled_price > 0,
        constraint = vault_factory_info.base_asset == base_asset_mint.key(),
        constraint = vault_factory_info.quote_asset == quote_asset_mint.key(),
        constraint = vault_factory_info.emergency_mode == false
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

    #[account(
        mut,
        constraint = vault_quote_asset_treasury.mint == quote_asset_mint.key(), // quote asset mint
        constraint = vault_quote_asset_treasury.owner.key() == vault_info.key() // Authority set to vault PDA
    )]
    pub vault_quote_asset_treasury: Box<Account<'info, TokenAccount>>,


    // if put option is not exercised, taker will get her base tokens back at this account
    #[account(
        mut,
        constraint = taker_base_asset_account.owner.key() == initializer.key(),
        constraint = taker_base_asset_account.mint == base_asset_mint.key()
    )]
    pub taker_base_asset_account: Box<Account<'info, TokenAccount>>,

    // if put option is exercised, taker will get her quote tokens at this account
    #[account(
        mut,
        constraint = taker_quote_asset_account.owner.key() == initializer.key(),
        constraint = taker_quote_asset_account.mint == quote_asset_mint.key()
    )]
    pub taker_quote_asset_account: Box<Account<'info, TokenAccount>>,


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
    new_funding: u64
)]
pub struct TakerAdjustFundingPutOptionVault<'info> {
    #[account(
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.matured == false,
        constraint = vault_factory_info.base_asset == base_asset_mint.key(),
        constraint = vault_factory_info.quote_asset == quote_asset_mint.key(),
        constraint = vault_factory_info.emergency_mode == false
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
        constraint = vault_factory_info.emergency_mode == false
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
pub struct MakerSettlePutOption<'info> {
    #[account(
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.matured == true,
        constraint = vault_factory_info.settled_price > 0,
        constraint = vault_factory_info.base_asset == base_asset_mint.key(),
        constraint = vault_factory_info.quote_asset == quote_asset_mint.key(),
        constraint = vault_factory_info.emergency_mode == false
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
            "PutOptionMakerInfo".as_bytes().as_ref(),
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref(), 
            initializer.key().as_ref()
        ],
        bump,
        constraint = !put_option_maker_info.is_settled
    )]
    pub put_option_maker_info: Account<'info, PutOptionMakerInfo>,

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
        mut,
        constraint = vault_quote_asset_treasury.mint == quote_asset_mint.key(), // quote asset mint
        constraint = vault_quote_asset_treasury.owner.key() == vault_info.key() // Authority set to vault PDA
    )]
    pub vault_quote_asset_treasury: Box<Account<'info, TokenAccount>>,


    // if put option is exercised, maker will get the base tokens she bought at strike price at this account
    #[account(
        mut,
        constraint = maker_base_asset_account.owner.key() == initializer.key(),
        constraint = maker_base_asset_account.mint == base_asset_mint.key()
    )]
    pub maker_base_asset_account: Box<Account<'info, TokenAccount>>,

    // if put option is not exercised, maker will get her quote tokens back at this account
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
    pub associated_token_program: Program<'info, AssociatedToken>
}


#[derive(Accounts)]
pub struct Initialize {}

#[derive(Accounts)]
pub struct MakerActivatePutOptionEmergencyMode<'info> {
    #[account(
        mut,
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.matured == false,
        constraint = vault_factory_info.emergency_mode == false
    )]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    #[account(
        seeds=[
            "PutOptionVaultInfo".as_bytes().as_ref(), 
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref()
        ], bump,
        constraint = vault_info.factory_vault == vault_factory_info.key(),
    )]
    pub vault_info: Account<'info, PutOptionVaultInfo>,

    #[account(
        seeds=[
            "PutOptionMakerInfo".as_bytes().as_ref(),
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref(), 
            initializer.key().as_ref()
        ],
        bump,
        constraint = !put_option_maker_info.is_settled
    )]
    pub put_option_maker_info: Account<'info, PutOptionMakerInfo>,

    // Check if initializer is signer, mut is required to reduce lamports (fees)
    #[account(mut)]
    pub initializer: Signer<'info>,
    
    // System Program requred for deduction of lamports (fees)
    pub system_program: Program<'info, System>,

}


#[derive(Accounts)]
pub struct TakerActivatePutOptionEmergencyMode<'info> {
    #[account(
        mut,
        constraint = vault_factory_info.strike > 0,
        constraint = vault_factory_info.is_initialized == true,
        constraint = vault_factory_info.matured == false,
        constraint = vault_factory_info.emergency_mode == false
    )]
    pub vault_factory_info: Account<'info, PutOptionVaultFactoryInfo>,

    #[account(
        seeds=[
            "PutOptionVaultInfo".as_bytes().as_ref(), 
            vault_factory_info.key().as_ref(),
            vault_info.ord.to_le_bytes().as_ref()
        ], bump,
        constraint = vault_info.factory_vault == vault_factory_info.key(),
    )]
    pub vault_info: Account<'info, PutOptionVaultInfo>,

    #[account(
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


    // Check if initializer is signer, mut is required to reduce lamports (fees)
    #[account(mut)]
    pub initializer: Signer<'info>,
    
    // System Program requred for deduction of lamports (fees)
    pub system_program: Program<'info, System>,

}
