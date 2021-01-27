use crate::core::exchanges::common::{
    CurrencyCodePair, CurrencyPair, ExchangeErrorType, ExchangeId, ExchangeName,
};
use crate::core::orders::fill::{EventSourceType, OrderFill};
use crate::core::DateTime;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use smallstr::SmallString;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use uuid::Uuid;

type String16 = SmallString<[u8; 16]>;

#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize, Hash)]
pub enum OrderSide {
    Buy = 1,
    Sell = 2,
}

impl OrderSide {
    pub fn change_side(&self) -> OrderSide {
        match self {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        }
    }
}

pub trait OptionOrderSideEx {
    fn change_side_opt(&self) -> Option<OrderSide>;
}

impl OptionOrderSideEx for Option<OrderSide> {
    fn change_side_opt(&self) -> Option<OrderSide> {
        match self {
            None => None,
            Some(OrderSide::Buy) => Some(OrderSide::Sell),
            Some(OrderSide::Sell) => Some(OrderSide::Buy),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize, Hash)]
pub enum OrderRole {
    Maker = 1,
    Taker = 2,
}

impl From<OrderFillRole> for OrderRole {
    fn from(fill_role: OrderFillRole) -> Self {
        match fill_role {
            OrderFillRole::Maker => OrderRole::Maker,
            OrderFillRole::Taker => OrderRole::Taker,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize, Hash)]
pub enum OrderType {
    Unknown = 0,
    Limit = 1,
    Market = 2,
    StopLoss = 3,
    TrailingStop = 4,
    Liquidation = 5,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize, Hash)]
pub enum OrderExecutionType {
    None = 0,
    MakerOnly = 1,
}

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Clone, Serialize, Deserialize, Hash)]
#[serde(transparent)]
pub struct ClientOrderId(String16);

impl ClientOrderId {
    #[inline]
    pub fn new(client_order_id: String16) -> Self {
        ClientOrderId(client_order_id)
    }

    /// Extracts a string slice containing the entire string.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Extracts a string slice containing the entire string.
    #[inline]
    pub fn as_mut_str(&mut self) -> &mut str {
        self.0.as_mut_str()
    }
}

impl From<&str> for ClientOrderId {
    #[inline]
    fn from(value: &str) -> Self {
        ClientOrderId(String16::from_str(value))
    }
}

impl fmt::Display for ClientOrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Clone, Serialize, Deserialize, Hash)]
#[serde(transparent)]
pub struct ExchangeOrderId(String16);

impl ExchangeOrderId {
    #[inline]
    pub fn new(client_order_id: String16) -> Self {
        ExchangeOrderId(client_order_id)
    }

    /// Extracts a string slice containing the entire string.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Extracts a string slice containing the entire string.
    #[inline]
    pub fn as_mut_str(&mut self) -> &mut str {
        self.0.as_mut_str()
    }
}

impl From<&str> for ExchangeOrderId {
    #[inline]
    fn from(value: &str) -> Self {
        ExchangeOrderId(String16::from_str(value))
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize, Hash)]
pub enum OrderStatus {
    Creating = 1,
    Created = 2,
    FailedToCreate = 3,
    Canceling = 4,
    Canceled = 5,
    FailedToCancel = 6,
    Completed = 7,
}

impl Default for OrderStatus {
    fn default() -> Self {
        OrderStatus::Creating
    }
}

impl OrderStatus {
    pub fn is_finished(&self) -> bool {
        let status = *self;
        status == OrderStatus::FailedToCreate
            || status == OrderStatus::Canceled
            || status == OrderStatus::Completed
    }
}

/// Id for reserved amount
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReservationId(u64);

impl ReservationId {
    pub fn gen_new() -> Self {
        static RESERVATION_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

        let new_id = RESERVATION_ID_COUNTER.fetch_add(1, Ordering::AcqRel);
        ReservationId(new_id)
    }
}

/// Immutable part of order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderHeader {
    version: u32, // for migrations started from 1

    pub client_order_id: ClientOrderId,

    pub init_time: DateTime,

    pub exchange_id: ExchangeId,
    pub exchange_name: ExchangeName,

    pub currency_pair: CurrencyPair,
    pub currency_code_pair: CurrencyCodePair,

    pub order_type: OrderType,

    pub side: Option<OrderSide>,
    pub amount: Decimal,

    pub reservation_id: ReservationId,

    pub signal_id: Option<String>,
    pub strategy_name: String,
}

impl OrderHeader {
    pub fn get_version(&self) -> u32 {
        self.version
    }
    pub fn increment_version(&mut self) {
        self.version += 1;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSimpleProps {
    client_order_id: ClientOrderId,
    pub raw_price: Option<Decimal>,
    pub role: Option<OrderRole>,
    pub execution_type: Option<OrderExecutionType>,
    pub exchange_order_id: Option<ExchangeOrderId>,
    pub stop_loss_price: Decimal,
    pub trailing_stop_delta: Decimal,

    pub status: OrderStatus,

    pub finished_time: Option<DateTime>,
}

impl OrderSimpleProps {
    pub fn new(client_order_id: ClientOrderId, price: Option<Decimal>) -> OrderSimpleProps {
        Self {
            client_order_id,
            raw_price: price,
            role: None,
            execution_type: None,
            exchange_order_id: None,
            stop_loss_price: Default::default(),
            trailing_stop_delta: Default::default(),
            status: Default::default(),
            finished_time: None,
        }
    }

    pub fn client_order_id(&self) -> &ClientOrderId {
        &self.client_order_id
    }
    pub fn is_finished(&self) -> bool {
        self.status.is_finished()
    }
    pub fn price(&self) -> Decimal {
        if let Some(price) = self.raw_price {
            price
        } else {
            panic!(
                "Can't get price from order {}",
                self.client_order_id.as_str()
            )
        }
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize, Hash)]
pub enum OrderFillType {
    UserTrade = 1,
    Liquidation = 2,
    Funding = 3,
    ClosePosition = 4,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize, Hash)]
pub enum OrderFillRole {
    Maker = 1,
    Taker = 2,
}

impl From<OrderRole> for OrderFillRole {
    fn from(role: OrderRole) -> Self {
        match role {
            OrderRole::Maker => OrderFillRole::Maker,
            OrderRole::Taker => OrderFillRole::Taker,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct OrderFills {
    fills: Vec<OrderFill>,
    filled_amount: Decimal,
}

impl OrderFills {
    pub fn last_fill_received_time(&self) -> Option<DateTime> {
        self.fills.last().map(|x| x.receive_time())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderStatusChange {
    id: Uuid,
    status: OrderStatus,
    time: DateTime,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct OrderStatusHistory {
    status_changes: Vec<OrderStatusChange>,
}

/// Helping properties for trading engine internal use
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SystemInternalOrderProps {
    pub creation_event_source_type: Option<EventSourceType>,
    pub last_order_creation_status_request_time: Option<DateTime>,
    pub last_creation_error_type: Option<ExchangeErrorType>,
    pub last_creation_error_message: String,

    pub cancellation_event_source_type: Option<EventSourceType>,
    pub last_order_cancellation_status_request_time: Option<DateTime>,
    pub last_cancellation_error: Option<ExchangeErrorType>,

    #[serde(skip_serializing)]
    pub is_canceling_from_wait_cancel_order: bool,

    #[serde(skip_serializing)]
    pub canceled_not_from_wait_cancel_order: bool,

    #[serde(skip_serializing)]
    pub cancellation_event_was_raised: bool,

    pub last_order_trades_request_time: Option<DateTime>,

    pub handled_by_balance_recovery: bool,
    pub filled_amount_after_cancellation: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSnapshot {
    pub header: Arc<OrderHeader>,
    pub props: OrderSimpleProps,
    pub fills: OrderFills,
    pub status_history: OrderStatusHistory,
    pub internal_props: SystemInternalOrderProps,
}

impl OrderSnapshot {
    pub fn new(
        header: Arc<OrderHeader>,
        props: OrderSimpleProps,
        fills: OrderFills,
        status_history: OrderStatusHistory,
        internal_props: SystemInternalOrderProps,
    ) -> Self {
        OrderSnapshot {
            header,
            props,
            fills,
            status_history,
            internal_props,
        }
    }

    pub fn add_fill(&mut self, fill: OrderFill) {
        self.fills.filled_amount += fill.amount();
        self.fills.fills.push(fill);
    }

    pub fn set_status(&mut self, new_status: OrderStatus, time: DateTime) {
        self.props.status = new_status;
        self.status_history.status_changes.push(OrderStatusChange {
            id: Uuid::default(),
            status: new_status,
            time,
        })
    }
}