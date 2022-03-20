use num_enum::{ TryFromPrimitive};
use solana_program::program_error::ProgramError;
use thiserror::Error;

pub type TradeBotResult<T> = Result<T, TradeBotErrors>;

#[derive( Debug, PartialEq, Eq)]
pub enum TradeBotError {
    Errors(TradeBotErrors),


}
#[repr(u8)]
#[derive(Debug, Error, Clone, PartialEq, PartialOrd, Eq, TryFromPrimitive)]
pub enum TradeBotErrors {
    #[error("Instruction Is not known by the program(a.k.a me)")]
    UnknownInstruction ,
    #[error("Instruction data is invalid")]
    InvalidInstruction,
    #[error("The market address is already saved")]
    MarketAlreadyInitialized,
    #[error("The provided market is not initialized")]
    MarketNotKnown,
    #[error("You are not authorized to perform this action")]
    Unauthorized,
    #[error("Trader already exists")]
    TraderExists = 5,
    #[error("Not enough tokens")]
    InsufficientTokens,
    #[error("The limit for the maximum number of open orders is passed")]
    ExceededOpenOrdersLimit,
     #[error("There are no open trades on the market")]
    NoTradesFoundOnMarket,
    #[error("Price range already has an unfilled order")]
    PriceAlreadyTraded,
    #[error("Price is lower than stop loss price")]
    StopLossLimit = 10,
    #[error("Price value is set too low to place a valid trade")]
    ProfitTooLow,
    #[error("Program Error")]
    ProgramErr,
    #[error("Unknown error")]
    UnknownError,


}



impl From<TradeBotErrors> for ProgramError {
    fn from(e: TradeBotErrors) -> ProgramError {

        ProgramError::Custom(e as u8 as u32)
    }
}

impl From<ProgramError> for TradeBotErrors {
    fn from(_err: ProgramError) -> Self {
        TradeBotErrors::ProgramErr

    }
}

