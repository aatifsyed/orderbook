use std::{fmt::Debug, ops::ControlFlow};

use enum_as_inner::EnumAsInner;
use num::Unsigned;

pub trait OrderBookApi<QuantityT, PriceT, OrderIdT>
where
    QuantityT: Unsigned,
{
    fn conditional_buy<BuyAbortReasonT: Debug>(
        &mut self,
        quantity: QuantityT,
        unit_price: PriceT,
        condition: impl FnOnce(ConditionalBuyArgs<'_, OrderIdT>) -> ControlFlow<BuyAbortReasonT, ()>,
    ) -> BuyResult<QuantityT, OrderIdT, BuyAbortReasonT>;

    fn conditional_sell<SellAbortReasonT: Debug>(
        &mut self,
        quantity: QuantityT,
        unit_price: PriceT,
        condition: impl FnOnce(ConditionalSellArgs<'_, OrderIdT>) -> ControlFlow<SellAbortReasonT, ()>,
    ) -> SellResult<QuantityT, OrderIdT, SellAbortReasonT>;

    fn query(&self, id: OrderIdT) -> QueryResult<QuantityT, PriceT>;

    fn cancel(&mut self, id: OrderIdT) -> CancelResult;
}

pub struct ConditionalBuyArgs<'a, OrderIdT> {
    pub seller_id: &'a OrderIdT,
}

pub struct ConditionalSellArgs<'a, OrderIdT> {
    pub buyer_id: &'a OrderIdT,
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
        buyer: OrderIdT,
    },
    BuyerFullyExecuted {
        buyer: OrderIdT,
        sellers_remaining: QuantityT,
    },
    SellerFullyExecuted {
        buyer: OrderIdT,
        buyers_remaining: QuantityT,
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

pub trait ReportingOrderBookApi<QuantityT, PriceT, OrderIdT>:
    OrderBookApi<QuantityT, PriceT, OrderIdT>
where
    QuantityT: Unsigned,
{
    // most-generous first
    fn buys(&self) -> Vec<Order<QuantityT, PriceT, OrderIdT>>;
    // cheapest first
    fn sells(&self) -> Vec<Order<QuantityT, PriceT, OrderIdT>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Order<QuantityT, PriceT, OrderIdT> {
    pub quantity: QuantityT,
    pub unit_price: PriceT,
    pub id: OrderIdT,
}

pub trait UnconditionalOrderBookApi<QuantityT, PriceT, OrderIdT>:
    OrderBookApi<QuantityT, PriceT, OrderIdT>
where
    QuantityT: Unsigned,
{
    fn unconditional_buy(
        &mut self,
        quantity: QuantityT,
        unit_price: PriceT,
    ) -> UnconditionalBuyResult<QuantityT, OrderIdT>;
    fn unconditional_sell(
        &mut self,
        quantity: QuantityT,
        unit_price: PriceT,
    ) -> UnconditionalSellResult<QuantityT, OrderIdT>;
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
        buyer: OrderIdT,
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

impl<T, QuantityT, PriceT, OrderIdT> UnconditionalOrderBookApi<QuantityT, PriceT, OrderIdT> for T
where
    T: OrderBookApi<QuantityT, PriceT, OrderIdT>,
    QuantityT: Unsigned,
{
    fn unconditional_buy(
        &mut self,
        quantity: QuantityT,
        unit_price: PriceT,
    ) -> UnconditionalBuyResult<QuantityT, OrderIdT> {
        match self.conditional_buy(quantity, unit_price, |_| ControlFlow::<()>::Continue(())) {
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
        quantity: QuantityT,
        unit_price: PriceT,
    ) -> UnconditionalSellResult<QuantityT, OrderIdT> {
        match self.conditional_sell(quantity, unit_price, |_| ControlFlow::<()>::Continue(())) {
            SellResult::QuantityWasZero => UnconditionalSellResult::QuantityWasZero,
            SellResult::AbortedOnCondition { .. } => {
                unreachable!("conditional_sell was aborted but no condition was given")
            }
            SellResult::EnteredOrderBook { id } => UnconditionalSellResult::EnteredOrderBook { id },
            SellResult::MutualFullExecution { buyer: seller } => {
                UnconditionalSellResult::MutualFullExecution { buyer: seller }
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
                buyers_remaining: sellers_remaining,
            } => UnconditionalSellResult::SellerFullyExecuted {
                buyer,
                sellers_remaining,
            },
        }
    }
}
