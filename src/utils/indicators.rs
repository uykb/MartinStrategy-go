/// Calculate Average True Range (ATR)
pub fn calculate_atr(highs: &[f64], lows: &[f64], closes: &[f64], period: i32) -> f64 {
    let period = period as usize;
    
    if highs.len() < period + 1 || lows.len() < period + 1 || closes.len() < period + 1 {
        return 0.0;
    }

    let mut tr_values = Vec::new();
    
    // Calculate True Range for each period
    for i in 1..closes.len() {
        let high = highs[i];
        let low = lows[i];
        let prev_close = closes[i - 1];
        
        let tr1 = high - low;
        let tr2 = (high - prev_close).abs();
        let tr3 = (low - prev_close).abs();
        
        let tr = tr1.max(tr2).max(tr3);
        tr_values.push(tr);
    }

    if tr_values.len() < period {
        return 0.0;
    }

    // Calculate Simple Moving Average of TR values
    let atr_sum: f64 = tr_values[tr_values.len() - period..].iter().sum();
    atr_sum / period as f64
}

/// Calculate Exponential Moving Average (EMA)
pub fn calculate_ema(values: &[f64], period: i32) -> Vec<f64> {
    let period = period as usize;
    if values.len() < period {
        return vec![];
    }

    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut ema = Vec::with_capacity(values.len());

    // First EMA is SMA
    let sma: f64 = values[0..period].iter().sum::<f64>() / period as f64;
    ema.push(sma);

    // Calculate subsequent EMAs
    for i in period..values.len() {
        let current_ema = (values[i] - ema.last().unwrap()) * multiplier + ema.last().unwrap();
        ema.push(current_ema);
    }

    ema
}

/// Calculate Simple Moving Average (SMA)
pub fn calculate_sma(values: &[f64], period: i32) -> f64 {
    let period = period as usize;
    if values.len() < period {
        return 0.0;
    }

    let sum: f64 = values[values.len() - period..].iter().sum();
    sum / period as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_atr() {
        let highs = vec![10.0, 12.0, 11.0, 13.0, 12.0, 14.0, 13.0, 15.0];
        let lows = vec![8.0, 9.0, 8.5, 10.0, 9.5, 11.0, 10.5, 12.0];
        let closes = vec![9.0, 11.0, 10.0, 12.0, 11.0, 13.0, 12.0, 14.0];
        
        let atr = calculate_atr(&highs, &lows, &closes, 3);
        assert!(atr > 0.0);
    }

    #[test]
    fn test_calculate_ema() {
        let values = vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0];
        let ema = calculate_ema(&values, 5);
        assert!(!ema.is_empty());
    }

    #[test]
    fn test_calculate_sma() {
        let values = vec![10.0, 11.0, 12.0, 13.0, 14.0];
        let sma = calculate_sma(&values, 5);
        assert_eq!(sma, 12.0);
    }
}
