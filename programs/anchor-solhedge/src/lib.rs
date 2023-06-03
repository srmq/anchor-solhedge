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
use solana_program::sysvar::clock::Clock;

declare_id!("8DYMPBKLDULX6G7ZuNrs1FcjuMqJwefu2MEfxkCq4sWY");

//const MAKER_COMISSION_PERCENT: f64 = 0.1;
//const TAKER_COMISSION_PERCENT: f64 = 0.1;

//Options will be negotiated up to 30 minutes to maturity
const FREEZE_SECONDS: u64 = 30*60;

//At this moment we will create options for at most
//30 days in the future
const MAX_MATURITY_FUTURE_SECONDS: u64 = 30*24*60*60;

#[program]
pub mod anchor_solhedge {
    use super::*;
    use anchor_spl::token::Transfer;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
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


        let wanted_amount = ctx.accounts.vault_info.lot_size.checked_mul(num_lots_to_sell).unwrap().checked_mul(ctx.accounts.vault_factory_info.strike).unwrap();

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
            ctx.accounts.vault_info.makers_total_pending_sell = ctx.accounts.vault_info.makers_total_pending_sell.checked_sub(decrease_amount).unwrap();
            ctx.accounts.vault_info.makers_total_pending_settle = ctx.accounts.vault_info.makers_total_pending_settle.checked_sub(decrease_amount).unwrap();
    
        }

        ctx.accounts.put_option_maker_info.premium_limit = premium_limit;        
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
        let transfer_amount = ctx.accounts.vault_info.lot_size.checked_mul(num_lots_to_sell).unwrap().checked_mul(ctx.accounts.vault_factory_info.strike).unwrap();
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
        let transfer_amount = params.lot_size.checked_mul(params.num_lots_to_sell).unwrap().checked_mul(params.strike).unwrap();
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
        associated_token::mint = quote_asset_mint, // Quote asset mint
        associated_token::authority = vault_info // Authority set to vault PDA
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
        associated_token::mint = quote_asset_mint, // Quote asset mint
        associated_token::authority = vault_info // Authority set to vault PDA
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
        constraint = maker_quote_asset_account.mint == quote_asset_mint.key(),
        constraint = vault_info.lot_size.checked_mul(num_lots_to_sell).unwrap().checked_mul(vault_factory_info.strike).unwrap() <= maker_quote_asset_account.amount
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
        constraint = maker_quote_asset_account.mint == quote_asset_mint.key(),
        constraint = params.lot_size.checked_mul(params.num_lots_to_sell).unwrap().checked_mul(params.strike).unwrap() <= maker_quote_asset_account.amount
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
    quote_asset: Pubkey
}

#[account]
pub struct PutOptionVaultInfo {
    factory_vault: Pubkey,

    ord: u64,
    max_makers: u16,
    max_takers: u16,
    lot_size: u64,

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

pub struct PutOptionTakerInfo {
    is_initialized: bool,
    
    ord: u16,
    max_base_asset: u64,
    qty_deposited: u64,
    is_settled: bool,
    owner: Pubkey,
    put_option_vault: Pubkey
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct MakerCreatePutOptionParams {
    maturity: u64, 
    strike: u64,
    max_makers: u16,
    max_takers: u16,
    lot_size: u64,
    num_lots_to_sell: u64,
    premium_limit: u64
}

#[error_code]
pub enum PutOptionError {
    #[msg("Number of max_makers cannot be zero")]
    MaxMakersZero,

    #[msg("Number of max_takers cannot be zero")]
    MaxTakersZero,

    #[msg("lot_size cannot be zero")]
    LotSizeZero,

    #[msg("num_lots_to_sell cannot be zero")]
    LotsToSellZero,

    #[msg("strike cannot be zero")]
    StrikeZero,

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

}    
