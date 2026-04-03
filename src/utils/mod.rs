pub mod logger;
pub mod indicators;

use rust_decimal::Decimal;
use rust_decimal::prelude::*;

pub fn round_to_tick_size(num: f64, tick_size: f64) -> f64 {
    if tick_size == 0.0 {
        return num;
    }
    
    let num_dec = Decimal::from_f64(num).unwrap_or_default();
    let tick_dec = Decimal::from_f64(tick_size).unwrap_or_default();
    
    let rounded = (num_dec / tick_dec).round() * tick_dec;
    rounded.to_f64().unwrap_or(num)
}

pub fn round_up_to_tick_size(num: f64, tick_size: f64) -> f64 {
    if tick_size == 0.0 {
        return num;
    }
    
    let num_dec = Decimal::from_f64(num).unwrap_or_default();
    let tick_dec = Decimal::from_f64(tick_size).unwrap_or_default();
    
    let rounded = (num_dec / tick_dec).ceil() * tick_dec;
    rounded.to_f64().unwrap_or(num)
}

pub fn to_fixed(num: f64, precision: i32) -> f64 {
    let num_dec = Decimal::from_f64(num).unwrap_or_default();
    let factor = Decimal::from_i64(10_i64.pow(precision as u32)).unwrap_or_default();
    let rounded = (num_dec * factor).round() / factor;
    rounded.to_f64().unwrap_or(num)
}
