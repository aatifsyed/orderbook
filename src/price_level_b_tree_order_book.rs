use crate::api::{
    BuyEntryOrExecution, BuyOrSell, Cancelled, ConditionalBuyArgs, ConditionalSellArgs,
    NoSuchOrder, Order, OrderBookApi, ReportingOrderBookApi, SellEntryOrExecution,
};
use crate::util::{BTreeMapExt as _, NonEmpty};
use num::Unsigned;
use numwit::Positive;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, VecDeque},
    fmt::Debug,
    ops::{self, ControlFlow},
};
use tap::Tap as _;

#[derive(Debug, Clone)]
pub struct PriceLevelBTreeOrderBook<QuantityT, PriceT, OrderIdT> {
    buys: BTreeMap<PriceT, NonEmpty<VecDeque<(OrderIdT, QuantityT)>>>,
    sells: BTreeMap<PriceT, NonEmpty<VecDeque<(OrderIdT, QuantityT)>>>,
    ids_to_price_level: HashMap<OrderIdT, BuyOrSellAtPriceLevel<PriceT>>,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum BuyOrSellAtPriceLevel<T> {
    Buy(T),
    Sell(T),
}

impl<QuantityT, PriceT> OrderBookApi<QuantityT, PriceT, uuid::Uuid>
    for PriceLevelBTreeOrderBook<QuantityT, PriceT, uuid::Uuid>
where
    QuantityT: Unsigned + Clone + Ord + Debug,
    PriceT: Clone + Ord + Debug + ops::Sub<Output = PriceT> + num::Zero,
{
    #[tracing::instrument(skip(self, condition), ret)]
    fn conditional_buy<BuyAbortReasonT: Debug>(
        &mut self,
        quantity: Positive<QuantityT>,
        unit_price: PriceT,
        condition: impl FnOnce(
            ConditionalBuyArgs<'_, uuid::Uuid>,
        ) -> std::ops::ControlFlow<BuyAbortReasonT, ()>,
    ) -> Result<BuyEntryOrExecution<QuantityT, PriceT, uuid::Uuid>, BuyAbortReasonT> {
        let quantity = quantity.into_inner();
        let entry_or_exc = match self.sells.first_entry() {
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
                    return Err(reason);
                }
                // a trade will occur
                let (ask_price, level) = ask_level.remove_entry();
                let (remaining_level, (seller_id, seller_quantity)) = level.pop_front();

                let spread = match ask_price.cmp(&unit_price) {
                    Ordering::Less => Some(Positive::new(unit_price - ask_price.clone()).unwrap()),
                    Ordering::Equal => None,
                    Ordering::Greater => unreachable!("already checked"),
                };

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
                        BuyEntryOrExecution::BuyerFullyExecuted {
                            seller: seller_id,
                            spread,
                            sellers_remaining,
                        }
                    }
                    Ordering::Equal => {
                        self.ids_to_price_level.remove(&seller_id);
                        if let Some(remaining_level) = remaining_level {
                            self.sells.insert_uncontended(ask_price, remaining_level)
                        }
                        BuyEntryOrExecution::MutualFullExecution {
                            seller: seller_id,
                            spread,
                        }
                    }
                    // buyer (us) wants more than the seller has
                    Ordering::Greater => {
                        let buyers_remaining = seller_quantity - quantity;
                        self.ids_to_price_level.remove(&seller_id);
                        if let Some(remaining_level) = remaining_level {
                            self.sells.insert_uncontended(ask_price, remaining_level)
                        }
                        BuyEntryOrExecution::SellerFullyExecuted {
                            seller: seller_id,
                            spread,
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
                    .or_insert(BuyOrSellAtPriceLevel::Buy(unit_price));
                BuyEntryOrExecution::EnteredOrderBook { id }
            }
        };
        Ok(entry_or_exc)
    }

    #[tracing::instrument(skip(self, condition), ret)]
    fn conditional_sell<SellAbortReasonT: Debug>(
        &mut self,
        quantity: Positive<QuantityT>,
        unit_price: PriceT,
        condition: impl FnOnce(
            ConditionalSellArgs<'_, uuid::Uuid>,
        ) -> std::ops::ControlFlow<SellAbortReasonT, ()>,
    ) -> Result<SellEntryOrExecution<QuantityT, PriceT, uuid::Uuid>, SellAbortReasonT> {
        let quantity = quantity.into_inner();
        let entry_or_exc = match self.buys.last_entry() {
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
                    return Err(reason);
                }
                // a trade will occur
                let (bid_price, level) = bid_level.remove_entry();
                let (remaining_level, (buyer_id, buyer_quantity)) = level.pop_front();

                let spread = match bid_price.cmp(&unit_price) {
                    Ordering::Less => unreachable!("already checked"),
                    Ordering::Equal => None,
                    Ordering::Greater => {
                        Some(Positive::new(bid_price.clone() - unit_price).unwrap())
                    }
                };

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
                        SellEntryOrExecution::SellerFullyExecuted {
                            buyer: buyer_id,
                            spread,
                            buyers_remaining,
                        }
                    }
                    Ordering::Equal => {
                        self.ids_to_price_level.remove(&buyer_id);
                        if let Some(remaining_level) = remaining_level {
                            self.buys.insert_uncontended(bid_price, remaining_level)
                        }
                        SellEntryOrExecution::MutualFullExecution {
                            buyer: buyer_id,
                            spread,
                        }
                    }
                    // seller (us) wants more than the buyer has
                    Ordering::Greater => {
                        let sellers_remaining = buyer_quantity - quantity;
                        self.ids_to_price_level.remove(&buyer_id);
                        if let Some(remaining_level) = remaining_level {
                            self.sells.insert_uncontended(bid_price, remaining_level)
                        }
                        SellEntryOrExecution::BuyerFullyExecuted {
                            buyer: buyer_id,
                            spread,
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
                    .or_insert(BuyOrSellAtPriceLevel::Sell(unit_price));
                SellEntryOrExecution::EnteredOrderBook { id }
            }
        };
        Ok(entry_or_exc)
    }

    #[tracing::instrument(skip(self), ret)]
    fn query(&self, id: uuid::Uuid) -> Result<BuyOrSell<QuantityT, PriceT>, NoSuchOrder> {
        match self.ids_to_price_level.get(&id) {
            Some(BuyOrSellAtPriceLevel::Buy(level)) => {
                let quantity = self
                    .buys
                    .get(level)
                    .expect("stale ids_to_price_level")
                    .iter()
                    .find_map(|(it_id, quantity)| match it_id == &id {
                        true => Some(quantity.clone()),
                        false => None,
                    })
                    .expect("stale ids_to_price_level");
                Ok(BuyOrSell::Buy {
                    quantity,
                    unit_price: level.clone(),
                })
            }
            Some(BuyOrSellAtPriceLevel::Sell(level)) => {
                let quantity = self
                    .sells
                    .get(level)
                    .expect("stale ids_to_price_level")
                    .iter()
                    .find_map(|(it_id, quantity)| match it_id == &id {
                        true => Some(quantity.clone()),
                        false => None,
                    })
                    .expect("stale ids_to_price_level");
                Ok(BuyOrSell::Sell {
                    quantity,
                    unit_price: level.clone(),
                })
            }
            None => Err(NoSuchOrder),
        }
    }

    #[tracing::instrument(skip(self), ret)]
    fn cancel(&mut self, id: uuid::Uuid) -> Result<Cancelled, NoSuchOrder> {
        match self.ids_to_price_level.remove(&id) {
            Some(BuyOrSellAtPriceLevel::Buy(price)) => {
                let level = self.buys.remove(&price).expect("stale ids_to_price_level");
                match level.pop_once_by(|(it_id, _)| it_id == &id) {
                    (Some(remaining_level), (_, _quantity)) => {
                        self.buys.insert_uncontended(price, remaining_level)
                    }
                    (None, (_, _quantity)) => {}
                }
                Ok(Cancelled)
            }
            Some(BuyOrSellAtPriceLevel::Sell(price)) => {
                let level = self.sells.remove(&price).expect("stale ids_to_price_level");
                match level.pop_once_by(|(it_id, _)| it_id == &id) {
                    (Some(remaining_level), (_, _quantity)) => {
                        self.buys.insert_uncontended(price, remaining_level)
                    }
                    (None, (_, _quantity)) => {}
                }
                Ok(Cancelled)
            }
            None => Err(NoSuchOrder),
        }
    }
}

impl<QuantityT, PriceT> ReportingOrderBookApi<QuantityT, PriceT, uuid::Uuid>
    for PriceLevelBTreeOrderBook<QuantityT, PriceT, uuid::Uuid>
where
    QuantityT: Unsigned + Clone + Ord + Debug,
    PriceT: Clone + Ord + Debug + ops::Sub<Output = PriceT> + num::Zero,
{
    fn buys(&self) -> Vec<Order<QuantityT, PriceT, uuid::Uuid>> {
        self.buys
            .iter()
            .rev()
            .flat_map(|(price, level)| {
                level.iter().map(|(id, quantity)| Order {
                    quantity: quantity.clone(),
                    unit_price: price.clone(),
                    id: *id,
                })
            })
            .collect()
    }

    fn sells(&self) -> Vec<Order<QuantityT, PriceT, uuid::Uuid>> {
        self.sells
            .iter()
            .flat_map(|(price, level)| {
                level.iter().map(|(id, quantity)| Order {
                    quantity: quantity.clone(),
                    unit_price: price.clone(),
                    id: *id,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::PriceLevelBTreeOrderBook;

    macro_rules! do_test_suite {
        ($ty:ty {
            $($fn_name:ident),* $(,)?
        }) => {
            $(
                #[test_log::test]
                fn $fn_name() {
                    crate::test_suite::$fn_name::<$ty, _, _, _>();
                }
            )*
        };
    }

    do_test_suite! {PriceLevelBTreeOrderBook<usize, usize, uuid::Uuid> {
        default_is_empty,
        add_query_remove_single_buy_order,
        add_query_remove_single_sell_order,
        single_resident_buy_is_fully_executed,
        single_resident_sell_is_fully_executed,
        buys_reported_with_price_time_priority,
        sells_reported_with_price_time_priority,
        buys_execute_with_price_time_priority,
        sells_execute_with_price_time_priority,
    }}
}
