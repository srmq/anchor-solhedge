use anchor_lang::{prelude::*, solana_program};
use anchor_spl::token::{self, Mint, Token, TokenAccount, MintTo};
use anchor_spl::associated_token::AssociatedToken;
use solana_program::{pubkey, pubkey::Pubkey};


declare_id!("2dAPtThes6YDdLL7bHUMPSduce9rKmnobSP8fQ4X5yTS");

// Mint address for SnakeDollar, get it from snake-tokens/snD.json ("mint")
const SND_MINT_ADDRESS: Pubkey = pubkey!("4QMc1CGEjnN3hgPRBWHJeExoWY6k7Pf5cArEnQ7QRdF9");

// Mint address for SnakeBTC, get it from snake-tokens/snBTC.json ("mint")
const SNBTC_MINT_ADDRESS: Pubkey = pubkey!("834NxSeQeT1XLcmcdsGXzKqhWcDRT4dLfdbzPt6hw5SP");

#[program]
pub mod snake_minter_devnet {
    use super::*;

    pub fn mint_snbtc(ctx: Context<MintSnBTC>) -> Result<()> {
        let amount_lamports = (0.02 * 10.0f64.powf(ctx.accounts.snake_btc_mint.decimals as f64)) as u64;
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.snake_btc_mint.to_account_info(),
                    to: ctx.accounts.user_snbtc_ata.to_account_info(),
                    authority: ctx.accounts.snake_mint_auth.to_account_info(),
                },
                &[&[
                    b"mint".as_ref(),
                    &[*ctx.bumps.get("snake_mint_auth").unwrap()],
                ]],
            ),
            amount_lamports,
        )?;

        Ok(())
    }

    pub fn mint_snd(ctx: Context<MintSnD>) -> Result<()> {
        let amount_lamports = 500 * (10.0f64.powf(ctx.accounts.snake_dollar_mint.decimals as f64) as u64);
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.snake_dollar_mint.to_account_info(),
                    to: ctx.accounts.user_snd_ata.to_account_info(),
                    authority: ctx.accounts.snake_mint_auth.to_account_info(),
                },
                &[&[
                    b"mint".as_ref(),
                    &[*ctx.bumps.get("snake_mint_auth").unwrap()],
                ]],
            ),
            amount_lamports,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct MintSnD<'info> {

    // mint for SnakeDollar
    #[account(
        mut,
        constraint = snake_dollar_mint.key() == SND_MINT_ADDRESS
    )]
    pub snake_dollar_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = user_snd_ata.mint == snake_dollar_mint.key(),
        constraint = user_snd_ata.owner.key() == initializer.key()
    )]
    pub user_snd_ata: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        seeds=[
            "mint".as_bytes().as_ref(), 
        ], bump,
        payer = initializer,
        space = std::mem::size_of::<SnakeMintAuthority>() + 8,
    )]
    pub snake_mint_auth: Account<'info, SnakeMintAuthority>,

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
pub struct MintSnBTC<'info> {

    // mint for SnakeDollar
    #[account(
        mut,
        constraint = snake_btc_mint.key() == SNBTC_MINT_ADDRESS
    )]
    pub snake_btc_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = user_snbtc_ata.mint == snake_btc_mint.key(),
        constraint = user_snbtc_ata.owner.key() == initializer.key()
    )]
    pub user_snbtc_ata: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        seeds=[
            "mint".as_bytes().as_ref(), 
        ], bump,
        payer = initializer,
        space = std::mem::size_of::<SnakeMintAuthority>() + 8,
    )]
    pub snake_mint_auth: Account<'info, SnakeMintAuthority>,

    // Check if initializer is signer, mut is required to reduce lamports (fees)
    #[account(mut)]
    pub initializer: Signer<'info>,

    // System Program requred for deduction of lamports (fees)
    pub system_program: Program<'info, System>,
    // Token Program required to call transfer instruction
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>
}


#[account]
pub struct SnakeMintAuthority {}
