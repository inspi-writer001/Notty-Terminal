// utils/helpers.rs
use anchor_lang::prelude::*;

/// Calculate cost to buy `token_amount` tokens using linear bonding curve
/// Formula: cost = base_price * amount + slope * (tokens_sold * amount + amount² / 2)
pub fn linear_buy_cost(
    base_price: u64,
    slope: u64,
    current_supply: u64,
    token_amount: u64,
) -> Option<u64> {
    // cost = base_price * token_amount + slope * (current_supply * token_amount + token_amount² / 2)

    let base_cost = (base_price as u128).checked_mul(token_amount as u128)?;

    let slope_component1 = (slope as u128)
        .checked_mul(current_supply as u128)?
        .checked_mul(token_amount as u128)?;

    let slope_component2 = (slope as u128)
        .checked_mul(token_amount as u128)?
        .checked_mul(token_amount as u128)?
        .checked_div(2)?;

    let total_cost = base_cost
        .checked_add(slope_component1)?
        .checked_add(slope_component2)?;

    // Convert back to u64, checking for overflow
    if total_cost <= u64::MAX as u128 {
        Some(total_cost as u64)
    } else {
        None
    }
}

/// Calculate refund for selling `token_amount` tokens using linear bonding curve
/// This is the reverse of the buy calculation
pub fn linear_sell_refund(
    base_price: u64,
    slope: u64,
    current_supply: u64,
    token_amount: u64,
) -> Option<u64> {
    // When selling, we're moving backwards on the curve
    // The new supply will be (current_supply - token_amount)
    let new_supply = current_supply.checked_sub(token_amount)?;

    // Refund = base_price * token_amount + slope * (new_supply * token_amount + token_amount² / 2)
    let base_refund = (base_price as u128).checked_mul(token_amount as u128)?;

    let slope_component1 = (slope as u128)
        .checked_mul(new_supply as u128)?
        .checked_mul(token_amount as u128)?;

    let slope_component2 = (slope as u128)
        .checked_mul(token_amount as u128)?
        .checked_mul(token_amount as u128)?
        .checked_div(2)?;

    let total_refund = base_refund
        .checked_add(slope_component1)?
        .checked_add(slope_component2)?;

    if total_refund <= u64::MAX as u128 {
        Some(total_refund as u64)
    } else {
        None
    }
}
