use std::{fmt::Debug, ops::ControlFlow};

use enum_as_inner::EnumAsInner;
use numwit::Positive;

pub trait OrderBookApi<QuantityT, PriceT, OrderIdT> {
    fn conditional_buy<BuyAbortReasonT: Debug>(
        &mut self,
        quantity: Positive<QuantityT>,
        unit_price: PriceT,
        condition: impl FnOnce(ConditionalBuyArgs<'_, OrderIdT>) -> ControlFlow<BuyAbortReasonT, ()>,
    ) -> Result<BuyEntryOrExecution<QuantityT, OrderIdT>, BuyAbortReasonT>;

    fn conditional_sell<SellAbortReasonT: Debug>(
        &mut self,
        quantity: Positive<QuantityT>,
        unit_price: PriceT,
        condition: impl FnOnce(ConditionalSellArgs<'_, OrderIdT>) -> ControlFlow<SellAbortReasonT, ()>,
    ) -> Result<SellEntryOrExecution<QuantityT, OrderIdT>, SellAbortReasonT>;

    fn query(&self, id: OrderIdT) -> Result<BuyOrSell<QuantityT, PriceT>, NoSuchOrder>;

    fn cancel(&mut self, id: OrderIdT) -> Result<Cancelled, NoSuchOrder>;
}

pub struct ConditionalBuyArgs<'a, OrderIdT> {
    pub seller_id: &'a OrderIdT,
}

pub struct ConditionalSellArgs<'a, OrderIdT> {
    pub buyer_id: &'a OrderIdT,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumAsInner)]
pub enum BuyEntryOrExecution<QuantityT, OrderIdT> {
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
pub enum SellEntryOrExecution<QuantityT, OrderIdT> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
#[error("No order found with that ID")]
pub struct NoSuchOrder;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cancelled;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumAsInner)]
pub enum BuyOrSell<QuantityT, PriceT> {
    Buy {
        quantity: QuantityT,
        unit_price: PriceT,
    },
    Sell {
        quantity: QuantityT,
        unit_price: PriceT,
    },
}

pub trait ReportingOrderBookApi<QuantityT, PriceT, OrderIdT>:
    OrderBookApi<QuantityT, PriceT, OrderIdT>
{
    /// most-generous first
    fn buys(&self) -> Vec<Order<QuantityT, PriceT, OrderIdT>>;
    /// cheapest first
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
{
    fn unconditional_buy(
        &mut self,
        quantity: Positive<QuantityT>,
        unit_price: PriceT,
    ) -> BuyEntryOrExecution<QuantityT, OrderIdT>;
    fn unconditional_sell(
        &mut self,
        quantity: Positive<QuantityT>,
        unit_price: PriceT,
    ) -> SellEntryOrExecution<QuantityT, OrderIdT>;
}

impl<T, QuantityT, PriceT, OrderIdT> UnconditionalOrderBookApi<QuantityT, PriceT, OrderIdT> for T
where
    T: OrderBookApi<QuantityT, PriceT, OrderIdT>,
{
    fn unconditional_buy(
        &mut self,
        quantity: Positive<QuantityT>,
        unit_price: PriceT,
    ) -> BuyEntryOrExecution<QuantityT, OrderIdT> {
        match self.conditional_buy(quantity, unit_price, |_| ControlFlow::<()>::Continue(())) {
            Ok(o) => o,
            Err(_) => {
                unreachable!("conditional_buy was aborted but no condition was given")
            }
        }
    }

    fn unconditional_sell(
        &mut self,
        quantity: Positive<QuantityT>,
        unit_price: PriceT,
    ) -> SellEntryOrExecution<QuantityT, OrderIdT> {
        match self.conditional_sell(quantity, unit_price, |_| ControlFlow::<()>::Continue(())) {
            Ok(o) => o,
            Err(_) => {
                unreachable!("conditional_sell was aborted but no condition was given")
            }
        }
    }
}
