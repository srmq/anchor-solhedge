use anchor_lang::prelude::*;

declare_id!("2dAPtThes6YDdLL7bHUMPSduce9rKmnobSP8fQ4X5yTS");

#[program]
pub mod snake_minter_devnet {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
