use num::{One, Zero};
use numwit::Positive;
use pretty_assertions::assert_eq;
use std::fmt::{self, Debug};

use crate::api::{
    BuyEntryOrExecution, BuyOrSell, Order, OrderBookApi, ReportingOrderBookApi,
    SellEntryOrExecution, UnconditionalOrderBookApi,
};

struct OrderMatcher<QuantityT, PriceT, OrderIdT> {
    quantity: Option<QuantityT>,
    unit_price: Option<PriceT>,
    id: Option<OrderIdT>,
}

impl<QuantityT, PriceT, OrderIdT> Debug for OrderMatcher<QuantityT, PriceT, OrderIdT>
where
    QuantityT: Debug,
    PriceT: Debug,
    OrderIdT: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn or_any<T: Debug>(o: &Option<T>) -> &dyn Debug {
            o.as_ref().map(|i| i as _).unwrap_or(&"<any>")
        }
        f.debug_struct("OrderMatcher")
            .field("quantity", or_any(&self.quantity))
            .field("unit_price", or_any(&self.unit_price))
            .field("id", or_any(&self.id))
            .finish()
    }
}

macro_rules! order {
    ($($field:ident = $value:expr),* $(,)?) => {
        #[allow(clippy::needless_update)]
        OrderMatcher {
            $($field: Some($value),)*
            ..OrderMatcher {
                quantity: None,
                unit_price: None,
                id: None,
            }
        }
    };
}

impl<QuantityT, PriceT, OrderIdT> PartialEq<Order<QuantityT, PriceT, OrderIdT>>
    for OrderMatcher<QuantityT, PriceT, OrderIdT>
where
    OrderIdT: PartialEq,
    QuantityT: PartialEq,
    PriceT: PartialEq,
{
    fn eq(&self, other: &Order<QuantityT, PriceT, OrderIdT>) -> bool {
        if let Some(quantity) = &self.quantity {
            if *quantity != other.quantity {
                return false;
            }
        }
        if let Some(unit_price) = &self.unit_price {
            if *unit_price != other.unit_price {
                return false;
            }
        }
        if let Some(id) = &self.id {
            if *id != other.id {
                return false;
            }
        }
        true
    }
}

fn one<T: One>() -> T {
    One::one()
}
fn two<T: One + Zero>() -> T {
    T::one() + one()
}
fn is_empty<T, QuantityT, PriceT, OrderIdT>(order_book: &T) -> bool
where
    T: ReportingOrderBookApi<QuantityT, PriceT, OrderIdT>,
{
    order_book.buys().is_empty() && order_book.sells().is_empty()
}
fn buy_unexecuted<T, QuantityT, PriceT, OrderIdT>(
    order_book: &mut T,
    quantity: QuantityT,
    unit_price: PriceT,
) -> OrderIdT
where
    T: OrderBookApi<QuantityT, PriceT, OrderIdT>,
    OrderIdT: Debug,
    QuantityT: Debug + PartialOrd + Zero,
    PriceT: Debug,
{
    order_book
        .unconditional_buy(Positive::new(quantity).unwrap(), unit_price)
        .into_entered_order_book()
        .expect("buy should not have executed")
}
fn sell_unexecuted<T, QuantityT, PriceT, OrderIdT>(
    order_book: &mut T,
    quantity: QuantityT,
    unit_price: PriceT,
) -> OrderIdT
where
    T: OrderBookApi<QuantityT, PriceT, OrderIdT>,
    OrderIdT: Debug,
    QuantityT: Debug + PartialOrd + Zero,
    PriceT: Debug,
{
    order_book
        .unconditional_sell(Positive::new(quantity).unwrap(), unit_price)
        .into_entered_order_book()
        .expect("sell should not have executed")
}

////////////////
// Test suite //
////////////////

pub fn default_is_empty<T, QuantityT, PriceT, OrderIdT>()
where
    T: ReportingOrderBookApi<QuantityT, PriceT, OrderIdT> + Default,
{
    assert!(is_empty(&T::default()))
}

pub fn add_query_remove_single_buy_order<T, QuantityT, PriceT, OrderIdT>()
where
    T: ReportingOrderBookApi<QuantityT, PriceT, OrderIdT> + Default,
    QuantityT: One + Debug + PartialEq + PartialOrd + Zero,
    PriceT: One + Debug + PartialEq,
    OrderIdT: Clone + Debug,
{
    let mut order_book = T::default();
    let id = buy_unexecuted(&mut order_book, one(), one());
    assert_eq!(
        Ok(BuyOrSell::Buy {
            quantity: one(),
            unit_price: one()
        }),
        order_book.query(id.clone()),
    );
    assert!(order_book.cancel(id.clone()).is_ok());
    assert!(order_book.query(id).is_err());
    assert!(is_empty(&order_book));
}

pub fn add_query_remove_single_sell_order<T, QuantityT, PriceT, OrderIdT>()
where
    T: ReportingOrderBookApi<QuantityT, PriceT, OrderIdT> + Default,
    QuantityT: One + Debug + PartialEq + PartialOrd + Zero,
    PriceT: One + Debug + PartialEq,
    OrderIdT: Clone + Debug,
{
    let mut order_book = T::default();
    let id = sell_unexecuted(&mut order_book, one(), one());
    assert_eq!(
        Ok(BuyOrSell::Sell {
            quantity: one(),
            unit_price: one()
        }),
        order_book.query(id.clone()),
    );
    assert!(order_book.cancel(id.clone()).is_ok());
    assert!(order_book.query(id).is_err());
    assert!(is_empty(&order_book));
}

pub fn single_resident_buy_is_fully_executed<T, QuantityT, PriceT, OrderIdT>()
where
    T: ReportingOrderBookApi<QuantityT, PriceT, OrderIdT> + Default,
    QuantityT: One + Debug + PartialEq + PartialOrd + Zero,
    PriceT: One + Debug + PartialEq,
    OrderIdT: Debug + PartialEq,
{
    let mut order_book = T::default();
    let resident_buy = buy_unexecuted(&mut order_book, one(), one());
    assert_eq!(
        SellEntryOrExecution::MutualFullExecution {
            buyer: resident_buy,
            spread: None
        },
        order_book.unconditional_sell(one(), one()),
    );
    assert!(is_empty(&order_book));
}

pub fn single_resident_sell_is_fully_executed<T, QuantityT, PriceT, OrderIdT>()
where
    T: ReportingOrderBookApi<QuantityT, PriceT, OrderIdT> + Default,
    QuantityT: One + Debug + PartialEq + PartialOrd + Zero,
    PriceT: One + Debug + PartialEq,
    OrderIdT: Debug + PartialEq,
{
    let mut order_book = T::default();
    let resident_sell = sell_unexecuted(&mut order_book, one(), one());
    assert_eq!(
        order_book.unconditional_buy(one(), one()),
        BuyEntryOrExecution::MutualFullExecution {
            seller: resident_sell,
            spread: None
        }
    );
    assert!(is_empty(&order_book));
}

pub fn buys_reported_with_price_time_priority<T, QuantityT, PriceT, OrderIdT>()
where
    T: ReportingOrderBookApi<QuantityT, PriceT, OrderIdT> + Default,
    QuantityT: One + Debug + PartialEq + PartialOrd + Zero,
    PriceT: One + Zero + Debug + PartialEq,
    OrderIdT: Debug + PartialEq,
{
    let mut order_book = T::default();
    let generous = buy_unexecuted(&mut order_book, one(), two());
    let miserly = buy_unexecuted(&mut order_book, one(), one());
    let generous_and_late = buy_unexecuted(&mut order_book, one(), two());
    assert_eq!(
        vec![
            order!(id = generous),
            order!(id = generous_and_late),
            order!(id = miserly)
        ],
        order_book.buys(),
    );
}

pub fn sells_reported_with_price_time_priority<T, QuantityT, PriceT, OrderIdT>()
where
    T: ReportingOrderBookApi<QuantityT, PriceT, OrderIdT> + Default,
    QuantityT: One + Debug + PartialEq + PartialOrd + Zero,
    PriceT: One + Zero + Debug + PartialEq,
    OrderIdT: Debug + PartialEq,
{
    let mut order_book = T::default();
    let cheap = sell_unexecuted(&mut order_book, one(), one());
    let expensive = sell_unexecuted(&mut order_book, one(), two());
    let cheap_and_late = sell_unexecuted(&mut order_book, one(), one());
    assert_eq!(
        vec![
            order!(id = cheap),
            order!(id = cheap_and_late),
            order!(id = expensive)
        ],
        order_book.sells(),
    );
}

pub fn buys_execute_with_price_time_priority<T, QuantityT, PriceT, OrderIdT>()
where
    T: ReportingOrderBookApi<QuantityT, PriceT, OrderIdT> + Default,
    QuantityT: One + Debug + PartialEq + PartialOrd + Zero,
    PriceT: One + Zero + Debug + PartialEq,
    OrderIdT: Debug + PartialEq,
{
    let mut order_book = T::default();
    let generous = buy_unexecuted(&mut order_book, one(), two());
    let miserly = buy_unexecuted(&mut order_book, one(), one());
    let generous_and_late = buy_unexecuted(&mut order_book, one(), two());
    assert_eq!(
        SellEntryOrExecution::MutualFullExecution {
            buyer: generous,
            spread: Some(Positive::new_unchecked(one()))
        },
        order_book.unconditional_sell(one(), one())
    );
    assert_eq!(
        SellEntryOrExecution::MutualFullExecution {
            buyer: generous_and_late,
            spread: Some(Positive::new_unchecked(one()))
        },
        order_book.unconditional_sell(one(), one())
    );
    assert_eq!(
        SellEntryOrExecution::MutualFullExecution {
            buyer: miserly,
            spread: None
        },
        order_book.unconditional_sell(one(), one())
    );
}

pub fn sells_execute_with_price_time_priority<T, QuantityT, PriceT, OrderIdT>()
where
    T: ReportingOrderBookApi<QuantityT, PriceT, OrderIdT> + Default,
    QuantityT: One + Debug + PartialEq + PartialOrd + Zero,
    PriceT: One + Zero + Debug + PartialEq,
    OrderIdT: Debug + PartialEq,
{
    let mut order_book = T::default();
    let cheap = sell_unexecuted(&mut order_book, one(), one());
    let expensive = sell_unexecuted(&mut order_book, one(), two());
    let cheap_and_late = sell_unexecuted(&mut order_book, one(), one());
    assert_eq!(
        BuyEntryOrExecution::MutualFullExecution {
            seller: cheap,
            spread: Some(Positive::new_unchecked(one()))
        },
        order_book.unconditional_buy(one(), two())
    );
    assert_eq!(
        BuyEntryOrExecution::MutualFullExecution {
            seller: cheap_and_late,
            spread: Some(Positive::new_unchecked(one()))
        },
        order_book.unconditional_buy(one(), two())
    );
    assert_eq!(
        BuyEntryOrExecution::MutualFullExecution {
            seller: expensive,
            spread: None
        },
        order_book.unconditional_buy(one(), two())
    );
}
