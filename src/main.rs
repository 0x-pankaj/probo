use std::{
    collections::{BTreeMap, VecDeque},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OptionType {
    Yes,
    No,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum OrderType {
    Buy,
    Sell,
}

#[derive(Clone, Debug)]
struct Order {
    id: u64,
    user_id: u32,
    option: OptionType,
    order_type: OrderType,
    price: f64,
    quantity: u32,
    timestamp: u64,
}

#[derive(Clone, Debug)]
struct Trade {
    buy_order_id: u64,
    sell_order_id: u64,
    option: OptionType,
    price: f64,
    quantity: u32,
}

pub struct OrderBook {
    option: OptionType,
    bids: BTreeMap<u64, VecDeque<Order>>,
    asks: BTreeMap<u64, VecDeque<Order>>,
}

impl OrderBook {
    fn new(option: OptionType) -> Self {
        OrderBook {
            option,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    fn price_to_cents(price: f64) -> u64 {
        (price * 100.0).round() as u64
    }

    fn add_order(&mut self, order: Order) {
        let price_cents = Self::price_to_cents(order.price);
        let orders = match order.order_type {
            OrderType::Buy => &mut self.bids,
            OrderType::Sell => &mut self.asks,
        };
        orders
            .entry(price_cents)
            .or_insert_with(VecDeque::new)
            .push_back(order);
    }

    fn remove_order(&mut self, order_type: OrderType, price: f64, order_id: u64) {
        let price_cents = Self::price_to_cents(price);
        let orders = match order_type {
            OrderType::Buy => &mut self.bids,
            OrderType::Sell => &mut self.asks,
        };
        if let Some(queue) = orders.get_mut(&price_cents) {
            queue.retain(|o| o.id != order_id);
            if queue.is_empty() {
                orders.remove(&price_cents);
            }
        }
    }
}

//structs for matching engine
struct MatchingEngine {
    yes_book: OrderBook,
    no_book: OrderBook,
    next_order_id: u64,
    commision_rate: f64, //eg 0.0223 -> 2.23 percentage
}

impl MatchingEngine {
    fn new() -> Self {
        MatchingEngine {
            yes_book: OrderBook::new(OptionType::Yes),
            no_book: OrderBook::new(OptionType::No),
            next_order_id: 1,
            commision_rate: 0.0223,
        }
    }

    //generate new order id
    fn generate_order_id(&mut self) -> u64 {
        let id = self.next_order_id;
        self.next_order_id += 1;
        id
    }

    //placing new order
    fn place_order(
        &mut self,
        user_id: u32,
        option: OptionType,
        order_type: OrderType,
        price: f64,
        quantity: u32,
    ) -> (Order, Vec<Trade>) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let option_clone = option.clone();
        let order = Order {
            id: self.generate_order_id(),
            user_id,
            option,
            order_type,
            price,
            quantity,
            timestamp,
        };
        let trades = self.match_order(&order);
        if order.quantity > 0 {
            let book = match option_clone {
                OptionType::Yes => &mut self.yes_book,
                OptionType::No => &mut self.no_book,
            };
            book.add_order(order.clone());
        }
        (order, trades)
    }

    fn match_order(&mut self, order: &Order) -> Vec<Trade> {
        let mut trades = Vec::new();
        let mut remaining_quantity = order.quantity;

        let book = match order.option {
            OptionType::Yes => &mut self.yes_book,
            OptionType::No => &mut self.no_book,
        };
        let counter_book = match order.option {
            OptionType::Yes => &mut self.no_book,
            OptionType::No => &mut self.yes_book,
        };

        //step 1 : try matching same option

        match order.order_type {
            OrderType::Buy => {
                while remaining_quantity > 0 {
                    if let Some((&ask_price_cents, asks)) = book.asks.iter_mut().next() {
                        let ask_price = ask_price_cents as f64 / 100.0;
                        if ask_price <= order.price {
                            if let Some(ask) = asks.pop_front() {
                                let matched_quantity = remaining_quantity.min(ask.quantity);
                                trades.push(Trade {
                                    buy_order_id: order.id,
                                    sell_order_id: ask.id,
                                    option: order.option,
                                    price: ask_price,
                                    quantity: matched_quantity,
                                });
                                remaining_quantity -= matched_quantity;
                                if ask.quantity > matched_quantity {
                                    let mut new_ask = ask.clone();
                                    new_ask.quantity -= matched_quantity;
                                    asks.push_front(new_ask);
                                }
                                if asks.is_empty() {
                                    book.asks.remove(&ask_price_cents);
                                }
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
            OrderType::Sell => {
                while remaining_quantity > 0 {
                    if let Some((&bid_price_cents, bids)) = book.bids.iter_mut().rev().next() {
                        let bid_price = bid_price_cents as f64 / 100.0;
                        if bid_price >= order.price {
                            if let Some(bid) = bids.pop_front() {
                                let matched_quantity = remaining_quantity.min(bid.quantity);
                                trades.push(Trade {
                                    buy_order_id: bid.id,
                                    sell_order_id: order.id,
                                    option: order.option,
                                    price: bid_price,
                                    quantity: matched_quantity,
                                });
                                remaining_quantity -= matched_quantity;
                                if bid.quantity > matched_quantity {
                                    let mut new_bid: Order = bid.clone();
                                    new_bid.quantity -= matched_quantity;
                                    bids.push_front(new_bid);
                                }
                                if bids.is_empty() {
                                    book.bids.remove(&bid_price_cents);
                                }
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        //step 2: oposite option
        if remaining_quantity > 0 {
            let counter_price = 10.0 - order.price;
            match order.order_type {
                OrderType::Buy => {
                    while remaining_quantity > 0 {
                        if let Some((&asks_price_cents, asks)) = counter_book.asks.iter_mut().next()
                        {
                            let ask_price = asks_price_cents as f64 / 100.0;
                            if ask_price <= counter_price {
                                if let Some(ask) = asks.pop_front() {
                                    let matched_quantity = remaining_quantity.min(ask.quantity);
                                    trades.push(Trade {
                                        buy_order_id: order.id,
                                        sell_order_id: ask.id,
                                        option: order.option,
                                        price: order.price,
                                        quantity: matched_quantity,
                                    });
                                    remaining_quantity -= matched_quantity;
                                    if ask.quantity > matched_quantity {
                                        let mut new_ask = ask.clone();
                                        new_ask.quantity -= matched_quantity;
                                        asks.push_front(new_ask);
                                    }
                                    if asks.is_empty() {
                                        counter_book.asks.remove(&asks_price_cents);
                                    }
                                }
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
                OrderType::Sell => {
                    while remaining_quantity > 0 {
                        if let Some((&bid_price_cents, bids)) =
                            counter_book.bids.iter_mut().rev().next()
                        {
                            let bid_price = bid_price_cents as f64 / 100.0;
                            if bid_price >= counter_price {
                                if let Some(bid) = bids.pop_front() {
                                    let matched_quantity = remaining_quantity.min(bid.quantity);
                                    trades.push(Trade {
                                        buy_order_id: bid.id,
                                        sell_order_id: order.id,
                                        option: order.option,
                                        price: order.price,
                                        quantity: matched_quantity,
                                    });
                                    remaining_quantity -= matched_quantity;
                                    if bid.quantity > matched_quantity {
                                        let mut new_bid = bid.clone();
                                        new_bid.quantity -= matched_quantity;
                                        bids.push_front(new_bid);
                                    }
                                    if bids.is_empty() {
                                        counter_book.bids.remove(&bid_price_cents);
                                    }
                                }
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
        }

        //step 3 : Market maker
        if remaining_quantity > 0 {
            trades.push(Trade {
                buy_order_id: if order.order_type == OrderType::Buy {
                    order.id
                } else {
                    0
                },
                sell_order_id: if order.order_type == OrderType::Sell {
                    order.id
                } else {
                    0
                },
                option: order.option,
                price: order.price,
                quantity: remaining_quantity,
            });
            remaining_quantity = 0;
        }

        trades
    }
}

fn main() {
    println!("Hello, world!");
}
