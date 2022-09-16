use std::ops::ControlFlow;

use enum_as_inner::EnumAsInner;
use num::Unsigned;

pub trait OrderBookApi<BuyAbortReasonT = (), SellAbortReasonT = ()> {
    type QuantityT: Unsigned;
    type PriceT;
    type OrderIdT;

    fn conditional_buy(
        &mut self,
        quantity: Self::QuantityT,
        unit_price: Self::PriceT,
        condition: impl FnOnce(
            Self::QuantityT,
            Self::PriceT,
            Self::OrderIdT,
        ) -> ControlFlow<BuyAbortReasonT, ()>,
    ) -> BuyResult<Self::QuantityT, Self::OrderIdT, BuyAbortReasonT>;

    fn conditional_sell(
        &mut self,
        quantity: Self::QuantityT,
        unit_price: Self::PriceT,
        condition: impl FnOnce(
            Self::QuantityT,
            Self::PriceT,
            Self::OrderIdT,
        ) -> ControlFlow<SellAbortReasonT, ()>,
    ) -> SellResult<Self::QuantityT, Self::OrderIdT, SellAbortReasonT>;

    fn query(&self, id: Self::OrderIdT) -> QueryResult<Self::QuantityT, Self::PriceT>;

    fn cancel(&self, id: Self::OrderIdT) -> CancelResult;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumAsInner)]
pub enum BuyResult<QuantityT, OrderIdT, BuyAbortReasonT> {
    QuantityWasZero,
    AbortedOnCondition {
        reason: BuyAbortReasonT,
    },
    EnteredOrderBook {
        id: OrderIdT,
    },
    MutualFullExecution {
        seller: OrderIdT,
    },
    BuyerFullyExecuted {
        seller: OrderIdT,
        sellers_remaining: QuantityT,
    },
    SellerFullyExecuted {
        seller: OrderIdT,
        buyers_remaining: QuantityT,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumAsInner)]
pub enum SellResult<QuantityT, OrderIdT, BuyAbortReasonT> {
    QuantityWasZero,
    AbortedOnCondition {
        reason: BuyAbortReasonT,
    },
    EnteredOrderBook {
        id: OrderIdT,
    },
    MutualFullExecution {
        seller: OrderIdT,
    },
    BuyerFullyExecuted {
        buyer: OrderIdT,
        sellers_remaining: QuantityT,
    },
    SellerFullyExecuted {
        buyer: OrderIdT,
        sellers_remaining: QuantityT,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumAsInner)]
pub enum QueryResult<QuantityT, PriceT> {
    NoSuchOrder,
    Buy {
        quantity: QuantityT,
        unit_price: PriceT,
    },
    Sell {
        quantity: QuantityT,
        unit_price: PriceT,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumAsInner)]
pub enum CancelResult {
    NoSuchOrder,
    Cancelled,
}

pub trait ReportingOrderBookApi: OrderBookApi {
    type BuyReport: Iterator<Item = Order<Self::QuantityT, Self::PriceT, Self::OrderIdT>>;
    type SellReport: Iterator<Item = Order<Self::QuantityT, Self::PriceT, Self::OrderIdT>>;
    // most-generous first
    fn buys(&self) -> Self::BuyReport;
    // cheapest first
    fn sells(&self) -> Self::SellReport;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Order<QuantityT, PriceT, OrderIdT> {
    quantity: QuantityT,
    unit_price: PriceT,
    id: OrderIdT,
}

pub trait UnconditionalOrderBookApi: OrderBookApi {
    fn unconditional_buy(
        &mut self,
        quantity: Self::QuantityT,
        unit_price: Self::PriceT,
    ) -> UnconditionalBuyResult<Self::QuantityT, Self::OrderIdT>;
    fn unconditional_sell(
        &mut self,
        quantity: Self::QuantityT,
        unit_price: Self::PriceT,
    ) -> UnconditionalSellResult<Self::QuantityT, Self::OrderIdT>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumAsInner)]
pub enum UnconditionalBuyResult<QuantityT, OrderIdT> {
    QuantityWasZero,
    EnteredOrderBook {
        id: OrderIdT,
    },
    MutualFullExecution {
        seller: OrderIdT,
    },
    BuyerFullyExecuted {
        seller: OrderIdT,
        sellers_remaining: QuantityT,
    },
    SellerFullyExecuted {
        seller: OrderIdT,
        buyers_remaining: QuantityT,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumAsInner)]
pub enum UnconditionalSellResult<QuantityT, OrderIdT> {
    QuantityWasZero,
    EnteredOrderBook {
        id: OrderIdT,
    },
    MutualFullExecution {
        seller: OrderIdT,
    },
    BuyerFullyExecuted {
        buyer: OrderIdT,
        sellers_remaining: QuantityT,
    },
    SellerFullyExecuted {
        buyer: OrderIdT,
        sellers_remaining: QuantityT,
    },
}

impl<T> UnconditionalOrderBookApi for T
where
    T: OrderBookApi,
{
    fn unconditional_buy(
        &mut self,
        quantity: Self::QuantityT,
        unit_price: Self::PriceT,
    ) -> UnconditionalBuyResult<Self::QuantityT, Self::OrderIdT> {
        match self.conditional_buy(quantity, unit_price, |_, _, _| ControlFlow::Continue(())) {
            BuyResult::QuantityWasZero => UnconditionalBuyResult::QuantityWasZero,
            BuyResult::AbortedOnCondition { .. } => {
                unreachable!("conditional_buy was aborted but no condition was given")
            }
            BuyResult::EnteredOrderBook { id } => UnconditionalBuyResult::EnteredOrderBook { id },
            BuyResult::MutualFullExecution { seller } => {
                UnconditionalBuyResult::MutualFullExecution { seller }
            }
            BuyResult::BuyerFullyExecuted {
                seller,
                sellers_remaining,
            } => UnconditionalBuyResult::BuyerFullyExecuted {
                seller,
                sellers_remaining,
            },
            BuyResult::SellerFullyExecuted {
                seller,
                buyers_remaining,
            } => UnconditionalBuyResult::SellerFullyExecuted {
                seller,
                buyers_remaining,
            },
        }
    }

    fn unconditional_sell(
        &mut self,
        quantity: Self::QuantityT,
        unit_price: Self::PriceT,
    ) -> UnconditionalSellResult<Self::QuantityT, Self::OrderIdT> {
        match self.conditional_sell(quantity, unit_price, |_, _, _| ControlFlow::Continue(())) {
            SellResult::QuantityWasZero => UnconditionalSellResult::QuantityWasZero,
            SellResult::AbortedOnCondition { .. } => {
                unreachable!("conditional_sell was aborted but no condition was given")
            }
            SellResult::EnteredOrderBook { id } => UnconditionalSellResult::EnteredOrderBook { id },
            SellResult::MutualFullExecution { seller } => {
                UnconditionalSellResult::MutualFullExecution { seller }
            }
            SellResult::BuyerFullyExecuted {
                buyer,
                sellers_remaining,
            } => UnconditionalSellResult::BuyerFullyExecuted {
                buyer,
                sellers_remaining,
            },
            SellResult::SellerFullyExecuted {
                buyer,
                sellers_remaining,
            } => UnconditionalSellResult::SellerFullyExecuted {
                buyer,
                sellers_remaining,
            },
        }
    }
}
