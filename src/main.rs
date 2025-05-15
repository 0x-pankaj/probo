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
        println!("remove order called");
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
            commision_rate: 0.0223, //this would be 2.23 percentage as a platform charge
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
        let mut order = Order {
            id: self.generate_order_id(),
            user_id,
            option,
            order_type,
            price,
            quantity,
            timestamp,
        };
        if order.quantity > 0 {
            let book = match option {
                OptionType::Yes => &mut self.yes_book,
                OptionType::No => &mut self.no_book,
            };

            if price < 0.5 || price > 9.5 {
                return (
                    Order {
                        quantity: 0,
                        ..order
                    },
                    Vec::new(),
                ); //reject order
            }
            book.add_order(order.clone());
        }
        let trades = self.match_order(&mut order);
        (order, trades)
    }

    fn cancel_order(
        &mut self,
        option: OptionType,
        order_type: OrderType,
        price: f64,
        order_id: u64,
    ) {
        let book = match option {
            OptionType::Yes => &mut self.yes_book,
            OptionType::No => &mut self.no_book,
        };
        book.remove_order(order_type, price, order_id);
    }

    fn match_order(&mut self, order: &mut Order) -> Vec<Trade> {
        let mut trades = Vec::new();
        let mut remaining_quantity = order.quantity;

        let book = if order.option == OptionType::Yes {
            &mut self.yes_book
        } else {
            &mut self.no_book
        };

        //step 1: try matching with same option book first
        println!("matching with same side");
        remaining_quantity = Self::match_with_book(book, order, remaining_quantity, &mut trades);
        println!("Cannot able to full fils");

        let book_for_counter = if order.option == OptionType::Yes {
            &mut self.no_book
        } else {
            &mut self.yes_book
        };

        // //error: cannot borrow self.yes_book as mutable more than once at a time
        // if order.option == OptionType::Yes {
        //     remaining_quantity =
        //         Self::match_with_book(&mut self.yes_book, order, remaining_quantity, &mut trades);
        // } else {
        //     remaining_quantity =
        //         Self::match_with_book(&mut self.no_book, order, remaining_quantity, &mut trades);
        // }

        println!("mathching with opposite side ");
        let counter_price = 10.0 - order.price;
        remaining_quantity = Self::match_with_counter_book(
            book_for_counter,
            order,
            remaining_quantity,
            counter_price,
            &mut trades,
        );

        println!("here end matching with opposite side ");

        //matching with opposite side same type then oposite type like YES buy with NO buy , YES sell with NO buy
        Self::match_with_counter_book_same_type(
            book_for_counter,
            order,
            remaining_quantity,
            counter_price,
            &mut trades,
        );

        //market maker

        // println!("now fulfilling with market maker");
        // if remaining_quantity > 0 {
        //     trades.push(Trade {
        //         buy_order_id: if order.order_type == OrderType::Buy {
        //             order.id
        //         } else {
        //             0
        //         },
        //         sell_order_id: if order.order_type == OrderType::Sell {
        //             order.id
        //         } else {
        //             0
        //         },
        //         option: order.option,
        //         price: order.price,
        //         quantity: remaining_quantity,
        //     });

        //     order.quantity = 0;
        // } else {
        //     order.quantity = remaining_quantity;
        // }

        trades
    }

    //Helper method to  match with the same option book
    fn match_with_book(
        book: &mut OrderBook,
        order: &mut Order,
        mut remaining_quantity: u32,
        trades: &mut Vec<Trade>,
    ) -> u32 {
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
                                    let mut new_bid = bid.clone();
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
        remaining_quantity
    }

    //method to match with the opposite option book
    fn match_with_counter_book(
        counter_book: &mut OrderBook,
        order: &mut Order,
        mut remaining_quantity: u32,
        counter_price: f64,
        trades: &mut Vec<Trade>,
    ) -> u32 {
        match order.order_type {
            OrderType::Buy => {
                while remaining_quantity > 0 {
                    if let Some((&asks_price_cents, asks)) = counter_book.asks.iter_mut().next() {
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
                        if bid_price <= counter_price {
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
        remaining_quantity
    }

    fn match_with_counter_book_same_type(
        counter_book: &mut OrderBook,
        order: &mut Order,
        mut remaining_quantity: u32,
        counter_price: f64,
        trades: &mut Vec<Trade>,
    ) {
        match order.order_type {
            OrderType::Buy => {
                while remaining_quantity > 0 {
                    if let Some((&bid_price_cents, bids)) =
                        counter_book.bids.iter_mut().rev().next()
                    {
                        let bid_price = bid_price_cents as f64 / 100.0;
                        if bid_price >= counter_price {
                            if let Some(bid) = bids.pop_front() {
                                let matched_quantity = remaining_quantity.min(bid.quantity);
                                trades.push(Trade {
                                    buy_order_id: order.id,
                                    sell_order_id: bid.id, //buy-to-buy match
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
            OrderType::Sell => {
                while remaining_quantity > 0 {
                    if let Some((&bid_price_cents, bids)) =
                        counter_book.bids.iter_mut().rev().next()
                    {
                        let bid_price = bid_price_cents as f64 / 100.0;
                        if bid_price <= counter_price {
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

    fn get_market_price(&self, option: OptionType) -> (Option<f64>, Option<f64>) {
        let book = match option {
            OptionType::Yes => &self.yes_book,
            OptionType::No => &self.no_book,
        };
        let bid_price = book
            .bids
            .iter()
            .rev()
            .next()
            .map(|(&p, _)| p as f64 / 100.0);
        let ask_price = book.asks.iter().next().map(|(&p, _)| p as f64 / 100.0);
        (bid_price, ask_price)
    }

    fn get_order_book(&self, option: OptionType) -> (BTreeMap<u64, u32>, BTreeMap<u64, u32>) {
        let book = match option {
            OptionType::Yes => &self.yes_book,
            OptionType::No => &self.no_book,
        };

        let bids: BTreeMap<u64, u32> = book
            .bids
            .iter()
            .map(|(&price, queue)| (price, queue.iter().map(|o| o.quantity).sum()))
            .collect();
        let asks: BTreeMap<u64, u32> = book
            .asks
            .iter()
            .map(|(&price, queue)| (price, queue.iter().map(|o| o.quantity).sum()))
            .collect();
        (bids, asks)
    }
}

fn main() {
    println!("Hello, world!");
    let mut engine = MatchingEngine::new();

    //scenario: buy Yes at 7.3, Buy No at 2.7
    println!("placing Buy yes at 7.3 (100 shares)");
    let (order1, trades1) = engine.place_order(1, OptionType::Yes, OrderType::Buy, 7.3, 150); //placed order
    // let (order11, trades11) = engine.place_order(11, OptionType::Yes, OrderType::Buy, 7.4, 150);
    println!("Order: {:?}", order1);
    println!("Trades: {:?},", trades1);

    let (bid_price, ask_price) = engine.get_market_price(OptionType::Yes);
    println!("bid: {:?}, ask: {:?}", bid_price, ask_price);

    let (bids, asks) = engine.get_order_book(OptionType::Yes);
    println!("bids: {:?}, asks: {:?}", bids, asks);

    let (order2, trades2) = engine.place_order(2, OptionType::No, OrderType::Sell, 2.9, 500); //placed sell order
    println!("Order2: {:?}", order2);
    println!("Trades2: {:?},", trades2);

    let (order3, trades3) = engine.place_order(3, OptionType::No, OrderType::Sell, 2.7, 500);
    println!("Order3: {:?}", order3);
    println!("Trades3: {:?},", trades3);

    let (bids, asks) = engine.get_order_book(OptionType::No);
    println!("bids: {:?}, asks: {:?}", bids, asks);

    // //removing partially order
    engine.cancel_order(OptionType::Yes, OrderType::Buy, 7.3, 1);

    let (order4, trades4) = engine.place_order(4, OptionType::No, OrderType::Sell, 7.3, 80);

    println!("Order: {:?}", order4);
    println!("Trades: {:?},", trades4);
}
// remove_order(&mut self, order_type: OrderType, price: f64, order_id: u64)
