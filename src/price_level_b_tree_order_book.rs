use crate::api::{
    BuyResult, CancelResult, ConditionalBuyArgs, ConditionalSellArgs, Order, OrderBookApi,
    QueryResult, ReportingOrderBookApi, SellResult,
};
use crate::util::{BTreeMapExt as _, NonEmpty};
use num::Unsigned;
use std::fmt::Debug;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, VecDeque},
    ops::ControlFlow,
};
use tap::Tap as _;

pub struct PriceLevelBTreeOrderBook<QuantityT, PriceT, OrderIdT> {
    buys: BTreeMap<PriceT, NonEmpty<VecDeque<(OrderIdT, QuantityT)>>>,
    sells: BTreeMap<PriceT, NonEmpty<VecDeque<(OrderIdT, QuantityT)>>>,
    ids_to_price_level: HashMap<OrderIdT, BuyOrSell<PriceT>>,
}

impl<QuantityT, PriceT, OrderIdT> Default
    for PriceLevelBTreeOrderBook<QuantityT, PriceT, OrderIdT>
{
    fn default() -> Self {
        Self {
            buys: Default::default(),
            sells: Default::default(),
            ids_to_price_level: Default::default(),
        }
    }
}

enum BuyOrSell<T> {
    Buy(T),
    Sell(T),
}

impl<QuantityT, PriceT> OrderBookApi<QuantityT, PriceT, uuid::Uuid>
    for PriceLevelBTreeOrderBook<QuantityT, PriceT, uuid::Uuid>
where
    QuantityT: Unsigned + Clone + Ord + Debug,
    PriceT: Clone + Ord + Debug,
{
    #[tracing::instrument(skip(self, condition), ret)]
    fn conditional_buy<BuyAbortReasonT: Debug>(
        &mut self,
        quantity: QuantityT,
        unit_price: PriceT,
        condition: impl FnOnce(
            ConditionalBuyArgs<'_, uuid::Uuid>,
        ) -> std::ops::ControlFlow<BuyAbortReasonT, ()>,
    ) -> BuyResult<QuantityT, uuid::Uuid, BuyAbortReasonT> {
        if quantity.is_zero() {
            return BuyResult::QuantityWasZero;
        }
        match self.sells.first_entry() {
            // A trade could occur
            Some(ask_level)
                if {
                    let ask_price = ask_level.key();
                    ask_price <= &unit_price
                } =>
            {
                if let ControlFlow::Break(reason) = condition(ConditionalBuyArgs {
                    seller_id: &ask_level.get().front().0,
                }) {
                    return BuyResult::AbortedOnCondition { reason };
                }
                // a trade will occur
                let (ask_price, level) = ask_level.remove_entry();
                let (remaining_level, (seller_id, seller_quantity)) = level.pop_front();

                match quantity.cmp(&seller_quantity) {
                    // buyer (us) wants less than the seller has
                    Ordering::Less => {
                        let sellers_remaining = seller_quantity - quantity;
                        self.sells.insert_uncontended(
                            ask_price,
                            match remaining_level {
                                Some(remaining) => remaining.tap_mut(|it| {
                                    it.push_front((seller_id, sellers_remaining.clone()))
                                }),
                                None => NonEmpty::vecdeque((seller_id, sellers_remaining.clone())),
                            },
                        );
                        BuyResult::BuyerFullyExecuted {
                            seller: seller_id,
                            sellers_remaining,
                        }
                    }
                    Ordering::Equal => {
                        self.ids_to_price_level.remove(&seller_id);
                        if let Some(remaining_level) = remaining_level {
                            self.sells.insert_uncontended(ask_price, remaining_level)
                        }
                        BuyResult::MutualFullExecution { seller: seller_id }
                    }
                    // buyer (us) wants more than the seller has
                    Ordering::Greater => {
                        let buyers_remaining = seller_quantity - quantity;
                        self.ids_to_price_level.remove(&seller_id);
                        if let Some(remaining_level) = remaining_level {
                            self.sells.insert_uncontended(ask_price, remaining_level)
                        }
                        BuyResult::SellerFullyExecuted {
                            seller: seller_id,
                            buyers_remaining,
                        }
                    }
                }
            }
            // Ask is too high, or no sellers
            Some(_) | None => {
                let id = uuid::Uuid::new_v4();
                self.buys
                    .entry(unit_price.clone())
                    .and_modify(|level| level.push_back((id, quantity.clone())))
                    .or_insert_with(|| NonEmpty::vecdeque((id, quantity)));
                self.ids_to_price_level
                    .entry(id)
                    .and_modify(|_| panic!("uuid collision"))
                    .or_insert(BuyOrSell::Buy(unit_price));
                BuyResult::EnteredOrderBook { id }
            }
        }
    }

    #[tracing::instrument(skip(self, condition), ret)]
    fn conditional_sell<SellAbortReasonT: Debug>(
        &mut self,
        quantity: QuantityT,
        unit_price: PriceT,
        condition: impl FnOnce(
            ConditionalSellArgs<'_, uuid::Uuid>,
        ) -> std::ops::ControlFlow<SellAbortReasonT, ()>,
    ) -> SellResult<QuantityT, uuid::Uuid, SellAbortReasonT> {
        if quantity.is_zero() {
            return SellResult::QuantityWasZero;
        }
        match self.buys.last_entry() {
            // A trade could occur
            Some(bid_level)
                if {
                    let bid_price = bid_level.key();
                    bid_price >= &unit_price
                } =>
            {
                if let ControlFlow::Break(reason) = condition(ConditionalSellArgs {
                    buyer_id: &bid_level.get().front().0,
                }) {
                    return SellResult::AbortedOnCondition { reason };
                }
                // a trade will occur
                let (bid_price, level) = bid_level.remove_entry();
                let (remaining_level, (buyer_id, buyer_quantity)) = level.pop_front();

                match quantity.cmp(&buyer_quantity) {
                    // seller (us) wants less than the buyer has
                    Ordering::Less => {
                        let buyers_remaining = buyer_quantity - quantity;
                        self.sells.insert_uncontended(
                            bid_price,
                            match remaining_level {
                                Some(remaining_level) => remaining_level.tap_mut(|it| {
                                    it.push_front((buyer_id, buyers_remaining.clone()))
                                }),
                                None => NonEmpty::vecdeque((buyer_id, buyers_remaining.clone())),
                            },
                        );
                        SellResult::SellerFullyExecuted {
                            buyer: buyer_id,
                            buyers_remaining,
                        }
                    }
                    Ordering::Equal => {
                        self.ids_to_price_level.remove(&buyer_id);
                        if let Some(remaining_level) = remaining_level {
                            self.buys.insert_uncontended(bid_price, remaining_level)
                        }
                        SellResult::MutualFullExecution { buyer: buyer_id }
                    }
                    // seller (us) wants more than the buyer has
                    Ordering::Greater => {
                        let sellers_remaining = buyer_quantity - quantity;
                        self.ids_to_price_level.remove(&buyer_id);
                        if let Some(remaining_level) = remaining_level {
                            self.sells.insert_uncontended(bid_price, remaining_level)
                        }
                        SellResult::BuyerFullyExecuted {
                            buyer: buyer_id,
                            sellers_remaining,
                        }
                    }
                }
            }
            // No bids are high enough, or no buyers
            Some(_) | None => {
                let id = uuid::Uuid::new_v4();
                self.sells
                    .entry(unit_price.clone())
                    .and_modify(|level| level.push_back((id, quantity.clone())))
                    .or_insert_with(|| NonEmpty::vecdeque((id, quantity)));
                self.ids_to_price_level
                    .entry(id)
                    .and_modify(|_| panic!("uuid collision"))
                    .or_insert(BuyOrSell::Sell(unit_price));
                SellResult::EnteredOrderBook { id }
            }
        }
    }

    #[tracing::instrument(skip(self), ret)]
    fn query(&self, id: uuid::Uuid) -> QueryResult<QuantityT, PriceT> {
        match self.ids_to_price_level.get(&id) {
            Some(BuyOrSell::Buy(level)) => {
                let quantity = self
                    .buys
                    .get(&level)
                    .expect("stale ids_to_price_level")
                    .iter()
                    .find_map(|(it_id, quantity)| match it_id == &id {
                        true => Some(quantity.clone()),
                        false => None,
                    })
                    .expect("stale ids_to_price_level");
                QueryResult::Buy {
                    quantity,
                    unit_price: level.clone(),
                }
            }
            Some(BuyOrSell::Sell(level)) => {
                let quantity = self
                    .sells
                    .get(&level)
                    .expect("stale ids_to_price_level")
                    .iter()
                    .find_map(|(it_id, quantity)| match it_id == &id {
                        true => Some(quantity.clone()),
                        false => None,
                    })
                    .expect("stale ids_to_price_level");
                QueryResult::Sell {
                    quantity,
                    unit_price: level.clone(),
                }
            }
            None => QueryResult::NoSuchOrder,
        }
    }

    #[tracing::instrument(skip(self), ret)]
    fn cancel(&self, id: uuid::Uuid) -> CancelResult {
        match self.ids_to_price_level.get(&id) {
            Some(BuyOrSell::Buy(_level)) => todo!(),
            Some(BuyOrSell::Sell(_level)) => todo!(),
            None => CancelResult::NoSuchOrder,
        }
    }
}

impl<QuantityT, PriceT> ReportingOrderBookApi<QuantityT, PriceT, uuid::Uuid>
    for PriceLevelBTreeOrderBook<QuantityT, PriceT, uuid::Uuid>
where
    QuantityT: Unsigned + Clone + Ord + Debug,
    PriceT: Clone + Ord + Debug,
{
    fn buys(&self) -> Vec<Order<QuantityT, PriceT, uuid::Uuid>> {
        self.buys
            .iter()
            .flat_map(|(price, level)| {
                level.iter().map(|(id, quantity)| Order {
                    quantity: quantity.clone(),
                    unit_price: price.clone(),
                    id: id.clone(),
                })
            })
            .collect()
    }

    fn sells(&self) -> Vec<Order<QuantityT, PriceT, uuid::Uuid>> {
        self.sells
            .iter()
            .rev()
            .flat_map(|(price, level)| {
                level.iter().map(|(id, quantity)| Order {
                    quantity: quantity.clone(),
                    unit_price: price.clone(),
                    id: id.clone(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;
    #[test]
    fn default_is_empty() {
        crate::test_suite::default_is_empty::<
            PriceLevelBTreeOrderBook<usize, usize, uuid::Uuid>,
            _,
            _,
            _,
        >();
    }
    #[test]
    fn zero_quantity_orders_are_rejected() {
        crate::test_suite::zero_quantity_orders_are_rejected::<
            PriceLevelBTreeOrderBook<usize, usize, uuid::Uuid>,
            _,
            _,
            _,
        >();
    }
    #[test]
    fn add_query_remove_single_buy_order() {
        crate::test_suite::add_query_remove_single_buy_order::<
            PriceLevelBTreeOrderBook<usize, usize, uuid::Uuid>,
            _,
            _,
            _,
        >();
    }
    #[test]
    fn add_query_remove_single_sell_order() {
        crate::test_suite::add_query_remove_single_sell_order::<
            PriceLevelBTreeOrderBook<usize, usize, uuid::Uuid>,
            _,
            _,
            _,
        >();
    }
    #[test]
    fn single_resident_buy_is_fully_executed() {
        crate::test_suite::single_resident_buy_is_fully_executed::<
            PriceLevelBTreeOrderBook<usize, usize, uuid::Uuid>,
            _,
            _,
            _,
        >();
    }
    #[test]
    fn single_resident_sell_is_fully_executed() {
        crate::test_suite::single_resident_sell_is_fully_executed::<
            PriceLevelBTreeOrderBook<usize, usize, uuid::Uuid>,
            _,
            _,
            _,
        >();
    }
    #[test]
    fn buys_have_price_time_priority() {
        crate::test_suite::buys_have_price_time_priority::<
            PriceLevelBTreeOrderBook<usize, usize, uuid::Uuid>,
            _,
            _,
            _,
        >();
    }
}
