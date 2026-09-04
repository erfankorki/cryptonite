//! Exchange domain models.

pub enum ExchangeName {
    Nobitex,
    Wallex,
    Bitpin,
}

pub struct Exchange {
    pub name: ExchangeName,
}

pub struct Wallet {
    pub exchange: Exchange,
}

pub struct Order {
    pub exchange: Exchange,
    pub market: Market,
}

pub struct Coin {
    pub symbol: String,
}

pub struct Market {
    pub src: Coin,
    pub dst: Coin,
}
