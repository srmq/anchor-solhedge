use anchor_lang::prelude::*;

#[error_code]
pub enum CallOptionError {
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
    OptionPremiumTooLow,

    #[msg("Insufficient time passed since maturity to activate emergency mode, please wait more")]
    EmergencyModeTooEarly
}    
