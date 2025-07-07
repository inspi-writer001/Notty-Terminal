#![allow(unexpected_cfgs)]

use std::str::FromStr;

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke, system_instruction};
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::metadata::{
    create_metadata_accounts_v3, mpl_token_metadata::types::DataV2, CreateMetadataAccountsV3,
    Metadata,
};
use anchor_spl::token::{
    self, spl_token, Mint, MintTo, SetAuthority, Token, TokenAccount, Transfer,
};
use anchor_spl::token_interface::{Mint as InterfaceMint, TokenInterface};
use raydium_launch_cpi::states::*;
use raydium_launch_cpi::{cpi, program::RaydiumLaunchpad};

pub const TOKEN_VAULT_SEED: &[u8] = b"token_vault";
pub const SOL_VAULT_SEED: &[u8] = b"sol_vault";
pub const MINT_AUTHORITY_SEED: &[u8] = b"mint_authority";
pub const VAULT_AUTHORITY_SEED: &[u8] = b"authority";
pub const FEE_VAULT_SEED: &[u8] = b"fee_vault";
pub const GLOBAL_STATE_SEED: &[u8] = b"global_state";

// // Bonding curve constants
// pub const TOTAL_SUPPLY: u64 = 1_000_000_000_000_000_000; // 1B tokens with 9 decimals
// pub const START_MARKET_CAP_SOL: u64 = 25_000_000_000; // 25 SOL in lamports
// pub const END_MARKET_CAP_SOL: u64 = 460_000_000_000; // 460 SOL in lamports
// pub const CREATION_FEE_SOL: u64 = 50_000_000; // 0.05 SOL in lamports

// Bonding curve constants - MUCH SMALLER FOR TESTING
pub const TOTAL_SUPPLY: u64 = 100_000_000_000_000_000; // 100M tokens with 9 decimals (was 100 tokens!)
pub const START_MARKET_CAP_SOL: u64 = 10_000_000; // 0.01 SOL start ✅
pub const END_MARKET_CAP_SOL: u64 = 1_000_000_000; // 1 SOL end ✅
pub const CREATION_FEE_SOL: u64 = 5_000_000; // 0.005 SOL fee ✅

const USDC_MINT: Pubkey = Pubkey::from_str_const("6mWfrWzYf5ot4S8Bti5SCDRnZWA5ABPH1SNkSq4mNN1C");
declare_id!("DLL2RN855xoMycXGipog3CjzQqpPBQJ6CpS6hPc8Tj1G");

#[account]
pub struct GlobalState {
    pub admin: Pubkey,
    pub total_fees_collected: u64,
    pub total_tokens_created: u64,
}

#[program]
pub mod notty_smart_contract {
    use anchor_lang::solana_program::program::invoke_signed;

    use super::*;

    pub fn initialize(ctx: Context<Initialize>, admin: Pubkey) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
        global_state.admin = admin;
        global_state.total_fees_collected = 0;
        global_state.total_tokens_created = 0;

        emit!(ProgramInitializedEvent {
            admin: admin,
            fee_vault: ctx.accounts.fee_vault.key(),
        });

        Ok(())
    }

    // Admin function to update the admin (transfer ownership)
    pub fn update_admin(ctx: Context<UpdateAdmin>, new_admin: Pubkey) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;

        // Only current admin can update
        require!(
            ctx.accounts.current_admin.key() == global_state.admin,
            ErrorCode::UnauthorizedAdmin
        );

        let old_admin = global_state.admin;
        global_state.admin = new_admin;

        emit!(AdminUpdatedEvent {
            old_admin: old_admin,
            new_admin: new_admin,
        });

        Ok(())
    }

    pub fn get_fee_vault_balance(ctx: Context<GetFeeVaultBalance>) -> Result<u64> {
        Ok(ctx.accounts.fee_vault.lamports())
    }

    // Admin-only function to withdraw accumulated fees
    pub fn withdraw_fees(ctx: Context<WithdrawFees>, amount: u64) -> Result<()> {
        let global_state = &ctx.accounts.global_state;

        // Only admin can withdraw
        require!(
            ctx.accounts.admin.key() == global_state.admin,
            ErrorCode::UnauthorizedAdmin
        );

        // Check if fee vault has enough SOL
        require!(
            ctx.accounts.fee_vault.lamports() >= amount,
            ErrorCode::InsufficientFeeVaultBalance
        );

        // Transfer SOL from fee vault to admin
        invoke_signed(
            &system_instruction::transfer(
                &ctx.accounts.fee_vault.key(),
                &ctx.accounts.admin.key(),
                amount,
            ),
            &[
                ctx.accounts.fee_vault.to_account_info(),
                ctx.accounts.admin.to_account_info(),
            ],
            &[&[FEE_VAULT_SEED, &[ctx.bumps.fee_vault]]],
        )?;

        emit!(FeesWithdrawnEvent {
            admin: ctx.accounts.admin.key(),
            amount: amount,
            fee_vault_remaining: ctx.accounts.fee_vault.lamports() - amount,
        });

        Ok(())
    }

    // UPDATED: Create bonding curve token with proper economics
    pub fn create_bonding_curve_token(
        ctx: Context<CreateBondingCurveToken>,
        token_name: String,
        token_symbol: String,
        token_uri: String,
    ) -> Result<()> {
        // 1. Charge creation fee (0.05 SOL)
        invoke(
            &system_instruction::transfer(
                &ctx.accounts.creator.key(),
                &ctx.accounts.fee_vault.key(),
                CREATION_FEE_SOL,
            ),
            &[
                ctx.accounts.creator.to_account_info(),
                ctx.accounts.fee_vault.to_account_info(),
            ],
        )?;

        // 2. Create metadata
        create_metadata_accounts_v3(
            CpiContext::new(
                ctx.accounts.token_metadata_program.to_account_info(),
                CreateMetadataAccountsV3 {
                    metadata: ctx.accounts.metadata_account.to_account_info(),
                    mint: ctx.accounts.mint_account.to_account_info(),
                    mint_authority: ctx.accounts.creator.to_account_info(),
                    update_authority: ctx.accounts.creator.to_account_info(),
                    payer: ctx.accounts.creator.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
            ),
            DataV2 {
                name: token_name.clone(),
                symbol: token_symbol.clone(),
                uri: token_uri.clone(),
                seller_fee_basis_points: 0,
                creators: None,
                collection: None,
                uses: None,
            },
            false,
            true,
            None,
        )?;

        // 3. Transfer mint authority to PDA
        let mint_key = ctx.accounts.mint_account.key();
        let (mint_authority, _) =
            Pubkey::find_program_address(&[MINT_AUTHORITY_SEED, mint_key.as_ref()], ctx.program_id);

        let cpi_accounts = SetAuthority {
            account_or_mint: ctx.accounts.mint_account.to_account_info(),
            current_authority: ctx.accounts.creator.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::set_authority(
            cpi_ctx,
            token::spl_token::instruction::AuthorityType::MintTokens,
            Some(mint_authority),
        )?;

        // 4. Mint entire supply to vault
        let mint_to_accounts = MintTo {
            mint: ctx.accounts.mint_account.to_account_info(),
            to: ctx.accounts.token_vault.to_account_info(),
            authority: ctx.accounts.mint_authority.to_account_info(),
        };

        let signer_seeds: &[&[&[u8]]] = &[&[
            MINT_AUTHORITY_SEED,
            mint_key.as_ref(),
            &[ctx.bumps.mint_authority],
        ]];

        let mint_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            mint_to_accounts,
            signer_seeds,
        );

        token::mint_to(mint_ctx, TOTAL_SUPPLY)?;

        // 5. Seed initial liquidity (50% of creation fee = 0.025 SOL)
        let initial_liquidity = CREATION_FEE_SOL / 2;
        let mint_key = ctx.accounts.mint_account.key();
        invoke_signed(
            &system_instruction::transfer(
                &ctx.accounts.fee_vault.key(),
                &ctx.accounts.sol_vault.key(),
                initial_liquidity,
            ),
            &[
                ctx.accounts.fee_vault.to_account_info(),
                ctx.accounts.sol_vault.to_account_info(),
            ],
            &[&[FEE_VAULT_SEED, &[ctx.bumps.fee_vault]]],
        )?;

        // 6. Initialize bonding curve vault
        let vault = &mut ctx.accounts.vault_account;
        vault.mint = ctx.accounts.mint_account.key();
        vault.token_account = ctx.accounts.token_vault.key();
        vault.sol_vault = ctx.accounts.sol_vault.key();
        vault.authority = ctx.accounts.vault_authority.key();
        vault.creator = ctx.accounts.creator.key();
        vault.total_supply = TOTAL_SUPPLY;
        vault.tokens_sold = 0;
        vault.sol_raised = initial_liquidity;
        vault.graduated = false;
        vault.migrated = false;

        emit!(BondingCurveTokenCreatedEvent {
            token_name,
            token_symbol,
            token_uri,
            mint_address: ctx.accounts.mint_account.key(),
            creator: ctx.accounts.creator.key(),
            decimals: 9,
            total_supply: TOTAL_SUPPLY,
            creation_fee_paid: CREATION_FEE_SOL,
            initial_liquidity: initial_liquidity,
        });

        Ok(())
    }

    // UPDATED: Buy tokens using bonding curve pricing
    pub fn buy_tokens_bonding_curve(
        ctx: Context<BuyTokensBondingCurve>,
        max_sol_cost: u64,
        min_tokens_out: u64,
    ) -> Result<()> {
        let vault = &mut ctx.accounts.vault_account;

        require!(!vault.graduated, ErrorCode::AlreadyGraduated);

        // Calculate how many tokens we can buy with max_sol_cost
        let (tokens_to_buy, actual_cost) = vault.calculate_purchase(max_sol_cost)?;

        require!(tokens_to_buy >= min_tokens_out, ErrorCode::SlippageExceeded);
        require!(tokens_to_buy > 0, ErrorCode::InvalidAmount);

        // Calculate fees (2% total: 1% platform, 1% creator)
        let fee_bps = 200u128; // ← Changed to u128
        let total_fee = (actual_cost as u128 * fee_bps / 10_000u128) as u64; // ← Use u128 arithmetic
        let creator_fee = total_fee / 2;
        let platform_fee = total_fee - creator_fee;
        let net_to_vault = actual_cost - total_fee;

        // Check buyer has enough SOL
        require!(
            ctx.accounts.buyer.lamports() >= actual_cost,
            ErrorCode::InsufficientFunds
        );

        // Transfer SOL to vault (net amount)
        invoke(
            &system_instruction::transfer(
                &ctx.accounts.buyer.key(),
                &ctx.accounts.sol_vault.key(),
                net_to_vault,
            ),
            &[
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.sol_vault.to_account_info(),
            ],
        )?;

        // Transfer fees
        invoke(
            &system_instruction::transfer(
                &ctx.accounts.buyer.key(),
                &ctx.accounts.platform_fee_account.key(),
                platform_fee,
            ),
            &[
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.platform_fee_account.to_account_info(),
            ],
        )?;

        invoke(
            &system_instruction::transfer(&ctx.accounts.buyer.key(), &vault.creator, creator_fee),
            &[
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.creator.to_account_info(),
            ],
        )?;

        // Transfer tokens to buyer
        let cpi_accounts = Transfer {
            from: ctx.accounts.token_vault.to_account_info(),
            to: ctx.accounts.buyer_token_account.to_account_info(),
            authority: ctx.accounts.vault_authority.to_account_info(),
        };

        let mint_key = ctx.accounts.mint.key();
        let signer_seeds: &[&[&[u8]]] = &[&[
            VAULT_AUTHORITY_SEED,
            mint_key.as_ref(),
            &[ctx.bumps.vault_authority],
        ]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );

        token::transfer(cpi_ctx, tokens_to_buy)?;

        // Update vault state
        vault.tokens_sold += tokens_to_buy;
        vault.sol_raised += net_to_vault;

        // Check for graduation
        let current_market_cap = vault.calculate_current_market_cap();
        if current_market_cap >= END_MARKET_CAP_SOL && !vault.graduated {
            vault.graduated = true;

            emit!(BondingCurveGraduatedEvent {
                mint: vault.mint,
                final_market_cap: current_market_cap,
                total_sol_raised: vault.sol_raised,
                tokens_sold: vault.tokens_sold,
            });
        }

        emit!(BondingCurveTradeEvent {
            trade_type: 0, // Buy
            mint_address: ctx.accounts.mint.key(),
            user: ctx.accounts.buyer.key(),
            sol_amount: actual_cost,
            token_amount: tokens_to_buy,
            new_token_price: vault.get_current_token_price(),
            market_cap: current_market_cap,
        });

        Ok(())
    }

    // UPDATED: Sell tokens using bonding curve pricing
    pub fn sell_tokens_bonding_curve(
        ctx: Context<SellTokensBondingCurve>,
        token_amount: u64,
        min_sol_out: u64,
    ) -> Result<()> {
        let vault = &mut ctx.accounts.vault_account;

        require!(!vault.graduated, ErrorCode::AlreadyGraduated);
        require!(token_amount > 0, ErrorCode::InvalidAmount);
        require!(
            vault.tokens_sold >= token_amount,
            ErrorCode::InsufficientTokensSold
        );

        // Calculate SOL to return
        let sol_to_return = vault.calculate_sell_return(token_amount)?;

        require!(sol_to_return >= min_sol_out, ErrorCode::SlippageExceeded);

        // Calculate fees (2% total: 1% platform, 1% creator)
        let fee_bps = 200;
        let total_fee = sol_to_return * fee_bps / 10_000;

        let creator_fee = total_fee / 2;
        let platform_fee = total_fee - creator_fee;
        let net_to_seller = sol_to_return - total_fee;

        // Ensure vault has enough SOL
        require!(
            ctx.accounts.sol_vault.lamports() >= sol_to_return,
            ErrorCode::VaultInsufficientSol
        );

        // Transfer tokens from seller to vault
        let cpi_accounts = Transfer {
            from: ctx.accounts.seller_token_account.to_account_info(),
            to: ctx.accounts.token_vault.to_account_info(),
            authority: ctx.accounts.seller.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, token_amount)?;

        // Transfer SOL to seller (net)
        let vault_seed = &[SOL_VAULT_SEED, &[ctx.bumps.sol_vault]];
        invoke_signed(
            &system_instruction::transfer(
                ctx.accounts.sol_vault.key,
                ctx.accounts.seller.key,
                net_to_seller,
            ),
            &[
                ctx.accounts.sol_vault.to_account_info(),
                ctx.accounts.seller.to_account_info(),
            ],
            &[vault_seed],
        )?;

        // Pay fees
        invoke_signed(
            &system_instruction::transfer(
                ctx.accounts.sol_vault.key,
                ctx.accounts.platform_fee_account.key,
                platform_fee,
            ),
            &[
                ctx.accounts.sol_vault.to_account_info(),
                ctx.accounts.platform_fee_account.to_account_info(),
            ],
            &[vault_seed],
        )?;

        invoke_signed(
            &system_instruction::transfer(ctx.accounts.sol_vault.key, &vault.creator, creator_fee),
            &[
                ctx.accounts.sol_vault.to_account_info(),
                ctx.accounts.creator.to_account_info(),
            ],
            &[vault_seed],
        )?;

        // Update vault state
        vault.tokens_sold -= token_amount;
        vault.sol_raised -= sol_to_return; // Total SOL that left the vault

        let current_market_cap = vault.calculate_current_market_cap();

        emit!(BondingCurveTradeEvent {
            trade_type: 1, // Sell
            mint_address: ctx.accounts.mint.key(),
            user: ctx.accounts.seller.key(),
            sol_amount: sol_to_return,
            token_amount: token_amount,
            new_token_price: vault.get_current_token_price(),
            market_cap: current_market_cap,
        });

        Ok(())
    }

    // Keep existing create_token for backward compatibility
    pub fn create_token(
        ctx: Context<CreateToken>,
        token_name: String,
        token_symbol: String,
        token_uri: String,
    ) -> Result<()> {
        create_metadata_accounts_v3(
            CpiContext::new(
                ctx.accounts.token_metadata_program.to_account_info(),
                CreateMetadataAccountsV3 {
                    metadata: ctx.accounts.metadata_account.to_account_info(),
                    mint: ctx.accounts.mint_account.to_account_info(),
                    mint_authority: ctx.accounts.payer.to_account_info(),
                    update_authority: ctx.accounts.payer.to_account_info(),
                    payer: ctx.accounts.payer.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
            ),
            DataV2 {
                name: token_name.clone(),
                symbol: token_symbol.clone(),
                uri: token_uri.clone(),
                seller_fee_basis_points: 0,
                creators: None,
                collection: None,
                uses: None,
            },
            false,
            true,
            None,
        )?;

        let cpi_accounts = SetAuthority {
            account_or_mint: ctx.accounts.mint_account.to_account_info(),
            current_authority: ctx.accounts.payer.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);

        token::set_authority(
            cpi_ctx,
            token::spl_token::instruction::AuthorityType::MintTokens,
            Some(ctx.accounts.mint_authority.key()),
        )?;

        emit!(TokenCreatedEvent {
            token_name,
            token_symbol,
            token_uri,
            mint_address: ctx.accounts.mint_account.key(),
            creator: ctx.accounts.payer.key(),
            decimals: 9,
        });

        Ok(())
    }

    // UPDATED: Migrate vault only if graduated
    pub fn migrate_vault(
        ctx: Context<MigrateVault>,
        base_mint_param: MintParams,
        curve_param: CurveParams,
        vesting_param: VestingParams,
    ) -> Result<()> {
        let vault = &mut ctx.accounts.vault_account;
        require!(!vault.migrated, ErrorCode::AlreadyMigrated);
        require!(vault.graduated, ErrorCode::NotGraduated);

        // Build the CPI account struct
        let cpi_accounts = cpi::accounts::Initialize {
            payer: ctx.accounts.payer.to_account_info(),
            creator: ctx.accounts.creator.to_account_info(),
            global_config: ctx.accounts.global_config.to_account_info(),
            platform_config: ctx.accounts.platform_config.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
            pool_state: ctx.accounts.pool_state.to_account_info(),
            base_mint: ctx.accounts.base_mint.to_account_info(),
            base_vault: ctx.accounts.base_vault.to_account_info(),
            quote_mint: ctx.accounts.quote_mint.to_account_info(),
            quote_vault: ctx.accounts.quote_vault.to_account_info(),
            metadata_account: ctx.accounts.metadata_account.to_account_info(),
            base_token_program: ctx.accounts.base_token_program.to_account_info(),
            quote_token_program: ctx.accounts.quote_token_program.to_account_info(),
            metadata_program: ctx.accounts.metadata_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            rent_program: ctx.accounts.rent_program.to_account_info(),
            event_authority: ctx.accounts.event_authority.to_account_info(),
            program: ctx.accounts.launchpad_program.to_account_info(),
        };

        cpi::initialize(
            CpiContext::new(
                ctx.accounts.launchpad_program.to_account_info(),
                cpi_accounts,
            ),
            base_mint_param,
            curve_param,
            vesting_param,
        );

        vault.migrated = true;
        emit!(VaultMigratedEvent {
            mint: vault.mint,
            pool: ctx.accounts.pool_state.key(),
            sol_deposited: ctx.accounts.quote_vault.to_account_info().lamports(),
        });

        Ok(())
    }
}

// UPDATED: Bonding curve vault structure
#[account]
pub struct BondingCurveVault {
    pub mint: Pubkey,
    pub authority: Pubkey,
    pub token_account: Pubkey,
    pub sol_vault: Pubkey,
    pub creator: Pubkey,
    pub total_supply: u64,
    pub tokens_sold: u64,
    pub sol_raised: u64,
    pub graduated: bool,
    pub migrated: bool,
}

impl BondingCurveVault {
    // Linear bonding curve pricing
    pub fn get_current_token_price(&self) -> u64 {
        if self.tokens_sold == 0 {
            return ((START_MARKET_CAP_SOL as u128 * 1_000_000_000) / TOTAL_SUPPLY as u128) as u64;
        }

        // Calculate where we are on the bonding curve (0.0 to 1.0)
        let progress = (self.tokens_sold as f64) / (TOTAL_SUPPLY as f64);

        // Linear interpolation between start and end market cap
        let price_range = END_MARKET_CAP_SOL - START_MARKET_CAP_SOL;
        let current_market_cap = START_MARKET_CAP_SOL + ((price_range as f64) * progress) as u64;

        // Price per token = current_market_cap / TOTAL_SUPPLY (not tokens_sold!)
        ((current_market_cap as u128 * 1_000_000_000) / TOTAL_SUPPLY as u128) as u64
    }

    pub fn calculate_current_market_cap(&self) -> u64 {
        if self.tokens_sold == 0 {
            return START_MARKET_CAP_SOL;
        }

        let progress = (self.tokens_sold as f64) / (TOTAL_SUPPLY as f64);
        let price_range = END_MARKET_CAP_SOL - START_MARKET_CAP_SOL;
        START_MARKET_CAP_SOL + ((price_range as f64) * progress) as u64
    }

    // pub fn calculate_purchase(&self, max_sol: u64) -> Result<(u64, u64)> {
    //     let tokens_remaining = TOTAL_SUPPLY - self.tokens_sold;

    //     // 🔍 DEBUG: Log purchase calculation
    //     msg!("calculate_purchase - max_sol: {}", max_sol);
    //     msg!(
    //         "calculate_purchase - tokens_sold: {}, tokens_remaining: {}",
    //         self.tokens_sold,
    //         tokens_remaining
    //     );

    //     require!(tokens_remaining > 0, ErrorCode::SoldOut);

    //     // Use u128 to prevent overflow in the estimate calculation
    //     let estimate_u128 = (max_sol as u128 * TOTAL_SUPPLY as u128) / END_MARKET_CAP_SOL as u128;
    //     let estimate = if estimate_u128 > u64::MAX as u128 {
    //         tokens_remaining
    //     } else {
    //         estimate_u128 as u64
    //     };

    //     let mut high = std::cmp::min(tokens_remaining, estimate);
    //     let mut low = 1u64;
    //     let mut best_tokens = 0u64;
    //     let mut best_cost = 0u64;

    //     // 🔍 DEBUG: Log binary search range
    //     msg!("Binary search - low: {}, high: {}", low, high);

    //     while low <= high {
    //         let mid = (low + high) / 2;
    //         if let Ok(cost) = self.calculate_buy_cost(mid) {
    //             msg!(
    //                 "Binary search - mid: {}, cost: {}, max_sol: {}",
    //                 mid,
    //                 cost,
    //                 max_sol
    //             );
    //             if cost <= max_sol {
    //                 best_tokens = mid;
    //                 best_cost = cost;
    //                 low = mid + 1;
    //             } else {
    //                 high = mid - 1;
    //             }
    //         } else {
    //             high = mid - 1;
    //         }
    //     }

    //     // 🔍 DEBUG: Log final result
    //     msg!(
    //         "calculate_purchase RESULT - tokens: {}, cost: {}",
    //         best_tokens,
    //         best_cost
    //     );

    //     require!(best_tokens > 0, ErrorCode::InsufficientFunds);
    //     Ok((best_tokens, best_cost))
    // }

    pub fn calculate_purchase(&self, max_sol: u64) -> Result<(u64, u64)> {
        let tokens_remaining = TOTAL_SUPPLY - self.tokens_sold;
        require!(tokens_remaining > 0, ErrorCode::SoldOut);

        msg!("calculate_purchase - max_sol: {}", max_sol);
        msg!(
            "calculate_purchase - tokens_sold: {}, tokens_remaining: {}",
            self.tokens_sold,
            tokens_remaining
        );

        // Binary search to find optimal token amount within SOL budget
        let mut low = 1u64;

        // 🛡️ SAFE: Use u128 to prevent overflow, then safely convert
        let conservative_estimate_u128 = (max_sol as u128).saturating_mul(100_000u128);
        let conservative_estimate = if conservative_estimate_u128 > u64::MAX as u128 {
            tokens_remaining / 1000 // Fallback: max 0.1% of remaining supply
        } else {
            std::cmp::min(conservative_estimate_u128 as u64, tokens_remaining / 1000)
        };

        let mut high = std::cmp::min(tokens_remaining, conservative_estimate);

        // 🛡️ EXTRA SAFETY: Ensure high is reasonable
        if high == 0 {
            high = std::cmp::min(1000u64, tokens_remaining);
        }

        let mut best_tokens = 0u64;
        let mut best_cost = 0u64;

        msg!("Binary search - low: {}, high: {}", low, high);

        while low <= high {
            let mid = (low + high) / 2;
            if let Ok(cost) = self.calculate_buy_cost(mid) {
                if cost <= max_sol {
                    best_tokens = mid;
                    best_cost = cost;
                    low = mid + 1;
                } else {
                    high = mid - 1;
                }
            } else {
                high = mid - 1;
            }
        }

        require!(best_tokens > 0, ErrorCode::InsufficientFunds);
        Ok((best_tokens, best_cost))
    }

    pub fn calculate_buy_cost(&self, token_amount: u64) -> Result<u64> {
        require!(
            self.tokens_sold + token_amount <= TOTAL_SUPPLY,
            ErrorCode::ExceedsSupply
        );

        let start_tokens = self.tokens_sold;
        let end_tokens = start_tokens + token_amount;

        let start_market_cap = self.get_market_cap_at_supply(start_tokens);
        let end_market_cap = self.get_market_cap_at_supply(end_tokens);

        let average_market_cap = (start_market_cap + end_market_cap) / 2;
        let cost =
            ((average_market_cap as u128 * token_amount as u128) / TOTAL_SUPPLY as u128) as u64;

        // 🚨 DEBUG: Show the calculation breakdown
        // msg!("=== CALCULATE_BUY_COST DEBUG ===");
        // msg!("token_amount: {}", token_amount);
        // msg!("start_market_cap: {}", start_market_cap);
        // msg!("end_market_cap: {}", end_market_cap);
        // msg!("average_market_cap: {}", average_market_cap);
        // msg!("TOTAL_SUPPLY: {}", TOTAL_SUPPLY);
        // msg!("calculated cost: {}", cost);

        Ok(cost)
    }

    // pub fn calculate_buy_cost(&self, token_amount: u64) -> Result<u64> {
    //     require!(
    //         self.tokens_sold + token_amount <= TOTAL_SUPPLY,
    //         ErrorCode::ExceedsSupply
    //     );

    //     let start_tokens = self.tokens_sold;
    //     let end_tokens = start_tokens + token_amount;

    //     // Linear curve integration: cost = (start_price + end_price) * amount / 2
    //     let start_market_cap = self.get_market_cap_at_supply(start_tokens);
    //     let end_market_cap = self.get_market_cap_at_supply(end_tokens);

    //     let average_market_cap = (start_market_cap + end_market_cap) / 2;
    //     let cost =
    //         ((average_market_cap as u128 * token_amount as u128) / TOTAL_SUPPLY as u128) as u64;

    //     Ok(cost)
    // }

    pub fn calculate_sell_return(&self, token_amount: u64) -> Result<u64> {
        require!(
            token_amount <= self.tokens_sold,
            ErrorCode::InsufficientTokensSold
        );

        let start_tokens = self.tokens_sold;
        let end_tokens = start_tokens - token_amount;

        let start_market_cap = self.get_market_cap_at_supply(start_tokens);
        let end_market_cap = self.get_market_cap_at_supply(end_tokens);

        let average_market_cap = (start_market_cap + end_market_cap) / 2;
        // Use u128 to prevent overflow in the multiplication
        let return_amount =
            ((average_market_cap as u128 * token_amount as u128) / TOTAL_SUPPLY as u128) as u64;

        Ok(return_amount)
    }

    fn get_market_cap_at_supply(&self, supply: u64) -> u64 {
        if supply == 0 {
            return START_MARKET_CAP_SOL;
        }

        let progress = (supply as f64) / (TOTAL_SUPPLY as f64);
        let price_range = END_MARKET_CAP_SOL - START_MARKET_CAP_SOL;
        START_MARKET_CAP_SOL + ((price_range as f64) * progress) as u64
    }
}

#[derive(Accounts)]
pub struct UpdateAdmin<'info> {
    #[account(mut)]
    pub current_admin: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED],
        bump
    )]
    pub global_state: Account<'info, GlobalState>,
}

#[derive(Accounts)]
pub struct GetFeeVaultBalance<'info> {
    #[account(
        seeds = [FEE_VAULT_SEED],
        bump
    )]
    /// CHECK: PDA fee vault
    pub fee_vault: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<GlobalState>(),
        seeds = [GLOBAL_STATE_SEED],
        bump
    )]
    pub global_state: Account<'info, GlobalState>,

    #[account(
        seeds = [FEE_VAULT_SEED],
        bump
    )]
    /// CHECK: PDA to collect fees
    pub fee_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [GLOBAL_STATE_SEED],
        bump
    )]
    pub global_state: Account<'info, GlobalState>,

    #[account(
        mut,
        seeds = [FEE_VAULT_SEED],
        bump
    )]
    /// CHECK: PDA fee vault
    pub fee_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[account]
pub struct TokenVault {
    pub mint: Pubkey,
    pub authority: Pubkey,
    pub token_account: Pubkey,
    pub sol_vault: Pubkey,
    pub price_per_token: u64,
    pub migrated: bool,
}

// UPDATED: Account structures for bonding curve
#[derive(Accounts)]
pub struct CreateBondingCurveToken<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        init,
        payer = creator,
        mint::decimals = 9,
        mint::authority = creator.key(),
        mint::freeze_authority = creator.key(),
    )]
    pub mint_account: Account<'info, Mint>,

    #[account(
        init,
        payer = creator,
        space = 8 + std::mem::size_of::<BondingCurveVault>(),
        seeds = [b"bonding_vault", mint_account.key().as_ref()],
        bump
    )]
    pub vault_account: Account<'info, BondingCurveVault>,

    #[account(
        init,
        payer = creator,
        seeds = [TOKEN_VAULT_SEED, mint_account.key().as_ref()],
        bump,
        token::mint = mint_account,
        token::authority = vault_authority
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [SOL_VAULT_SEED, mint_account.key().as_ref()],
        bump
    )]
    /// CHECK: PDA to receive SOL
    pub sol_vault: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [FEE_VAULT_SEED],
        bump
    )]
    /// CHECK: PDA to collect fees
    pub fee_vault: UncheckedAccount<'info>,

    #[account(
        seeds = [VAULT_AUTHORITY_SEED, mint_account.key().as_ref()],
        bump
    )]
    /// CHECK: PDA authority
    pub vault_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [MINT_AUTHORITY_SEED, mint_account.key().as_ref()],
        bump
    )]
    /// CHECK: PDA mint authority
    pub mint_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"metadata", token_metadata_program.key().as_ref(), mint_account.key().as_ref()],
        bump,
        seeds::program = token_metadata_program.key()
    )]
    /// CHECK: Metadata account
    pub metadata_account: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub token_metadata_program: Program<'info, Metadata>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct BuyTokensBondingCurve<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(mut)]
    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [TOKEN_VAULT_SEED, mint.key().as_ref()],
        bump
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"bonding_vault", mint.key().as_ref()],
        bump,
    )]
    pub vault_account: Account<'info, BondingCurveVault>,

    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = mint,
        associated_token::authority = buyer
    )]
    pub buyer_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [SOL_VAULT_SEED, mint.key().as_ref()],
        bump
    )]
    /// CHECK: Sol vault
    pub sol_vault: UncheckedAccount<'info>,

    #[account(
        seeds = [VAULT_AUTHORITY_SEED, mint.key().as_ref()],
        bump
    )]
    /// CHECK: vault authority
    pub vault_authority: UncheckedAccount<'info>,

    /// CHECK: Platform fee recipient
    #[account(mut)]
    pub platform_fee_account: UncheckedAccount<'info>,

    /// CHECK: Creator fee recipient
    #[account(mut)]
    pub creator: UncheckedAccount<'info>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SellTokensBondingCurve<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,

    #[account(mut)]
    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [TOKEN_VAULT_SEED, mint.key().as_ref()],
        bump
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"bonding_vault", mint.key().as_ref()],
        bump,
    )]
    pub vault_account: Account<'info, BondingCurveVault>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = seller
    )]
    pub seller_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [SOL_VAULT_SEED, mint.key().as_ref()],
        bump
    )]
    /// CHECK: Sol vault
    pub sol_vault: UncheckedAccount<'info>,

    /// CHECK: Platform fee recipient
    #[account(mut)]
    pub platform_fee_account: UncheckedAccount<'info>,

    /// CHECK: Creator fee recipient
    #[account(mut)]
    pub creator: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// Events
#[event]
pub struct BondingCurveTokenCreatedEvent {
    pub token_name: String,
    pub token_symbol: String,
    pub token_uri: String,
    pub mint_address: Pubkey,
    pub creator: Pubkey,
    pub decimals: u8,
    pub total_supply: u64,
    pub creation_fee_paid: u64,
    pub initial_liquidity: u64,
}

#[event]
pub struct BondingCurveTradeEvent {
    pub trade_type: u8, // 0 = buy, 1 = sell
    pub mint_address: Pubkey,
    pub user: Pubkey,
    pub sol_amount: u64,
    pub token_amount: u64,
    pub new_token_price: u64,
    pub market_cap: u64,
}

#[event]
pub struct BondingCurveGraduatedEvent {
    pub mint: Pubkey,
    pub final_market_cap: u64,
    pub total_sol_raised: u64,
    pub tokens_sold: u64,
}

// Keep existing events for backward compatibility
#[event]
pub struct TokenCreatedEvent {
    pub token_name: String,
    pub token_symbol: String,
    pub token_uri: String,
    pub mint_address: Pubkey,
    pub creator: Pubkey,
    pub decimals: u8,
}

#[event]
pub struct VaultMigratedEvent {
    pub mint: Pubkey,
    pub pool: Pubkey,
    pub sol_deposited: u64,
}

#[event]
pub struct ProgramInitializedEvent {
    pub admin: Pubkey,
    pub fee_vault: Pubkey,
}

#[event]
pub struct AdminUpdatedEvent {
    pub old_admin: Pubkey,
    pub new_admin: Pubkey,
}

#[event]
pub struct FeesWithdrawnEvent {
    pub admin: Pubkey,
    pub amount: u64,
    pub fee_vault_remaining: u64,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Not enough SOL to buy tokens")]
    InsufficientFunds,
    #[msg("Vault doesn't have enough SOL to refund")]
    VaultInsufficientSol,
    #[msg("Numerical overflow occurred")]
    NumericalOverflow,
    #[msg("Liquidity has already been migrated")]
    AlreadyMigrated,
    #[msg("Vault hasn't reached the migration threshold")]
    TargetNotReached,
    #[msg("Token amount exceeds available supply")]
    ExceedsSupply,
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
    #[msg("Invalid amount specified")]
    InvalidAmount,
    #[msg("All tokens have been sold")]
    SoldOut,
    #[msg("Insufficient tokens sold to support this sale")]
    InsufficientTokensSold,
    #[msg("Bonding curve has already graduated")]
    AlreadyGraduated,
    #[msg("Bonding curve has not graduated yet")]
    NotGraduated,
    #[msg("Only admin can perform this action")]
    UnauthorizedAdmin,
    #[msg("Fee vault doesn't have enough balance")]
    InsufficientFeeVaultBalance,
}

// Keep existing account structures for backward compatibility
#[derive(Accounts)]
pub struct CreateToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        payer = payer,
        mint::decimals = 9,
        mint::authority = payer,
        mint::freeze_authority = payer,
    )]
    pub mint_account: Account<'info, Mint>,

    /// CHECK: PDA metadata account
    #[account(
        mut,
        seeds = [b"metadata", token_metadata_program.key().as_ref(), mint_account.key().as_ref()],
        bump,
        seeds::program = token_metadata_program.key(),
    )]
    pub metadata_account: UncheckedAccount<'info>,

    #[account(seeds=[MINT_AUTHORITY_SEED, mint_account.key().as_ref()], bump)]
    /// CHECK: this is our PDA mint authority
    pub mint_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub token_metadata_program: Program<'info, Metadata>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

// Migration accounts (updated to use BondingCurveVault)
#[derive(Accounts)]
pub struct MigrateVault<'info> {
    pub launchpad_program: Program<'info, RaydiumLaunchpad>,

    #[account(
        mut,
        seeds = [b"bonding_vault", base_mint.key().as_ref()],
        bump,
    )]
    pub vault_account: Account<'info, BondingCurveVault>,

    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: Creator of the base token  
    #[account()]
    pub creator: UncheckedAccount<'info>,

    pub global_config: Box<Account<'info, GlobalConfig>>,
    pub platform_config: Box<Account<'info, PlatformConfig>>,

    /// CHECK: Authority PDA
    #[account(
      seeds = [raydium_launch_cpi::AUTH_SEED.as_bytes()],
      bump,
      seeds::program = launchpad_program.key(),
    )]
    pub authority: UncheckedAccount<'info>,

    /// CHECK: Pool state PDA  
    #[account(
      mut,
      seeds = [
        POOL_SEED.as_bytes(),
        base_mint.key().as_ref(),
        quote_mint.key().as_ref(),
      ],
      bump,
      seeds::program = launchpad_program.key(),
    )]
    pub pool_state: UncheckedAccount<'info>,

    #[account(mut)]
    pub base_mint: Signer<'info>,

    /// CHECK: Base vault
    #[account(
      mut,
      seeds = [
        POOL_VAULT_SEED.as_bytes(),
        pool_state.key().as_ref(),
        base_mint.key().as_ref(),
      ],
      bump,
      seeds::program = launchpad_program.key(),
    )]
    pub base_vault: UncheckedAccount<'info>,

    #[account(address = global_config.quote_mint, constraint = quote_mint.key() == USDC_MINT)]
    pub quote_mint: Box<InterfaceAccount<'info, InterfaceMint>>,

    /// CHECK: Quote vault
    #[account(
      mut,
      seeds = [
        POOL_VAULT_SEED.as_bytes(),
        pool_state.key().as_ref(),
        quote_mint.key().as_ref(),
      ],
      bump,
      seeds::program = launchpad_program.key(),
    )]
    pub quote_vault: UncheckedAccount<'info>,

    /// CHECK: Metadata account  
    #[account(mut)]
    pub metadata_account: UncheckedAccount<'info>,

    #[account(address = spl_token::id())]
    pub base_token_program: Interface<'info, TokenInterface>,
    pub quote_token_program: Program<'info, Token>,
    pub metadata_program: Program<'info, Metadata>,
    pub system_program: Program<'info, System>,
    pub rent_program: Sysvar<'info, Rent>,

    /// CHECK: Event authority  
    #[account(
      seeds = [b"__event_authority"],
      bump,
      seeds::program = launchpad_program.key(),
    )]
    pub event_authority: AccountInfo<'info>,

    /// CHECK: Program Account for Launch  
    #[account(address = launchpad_program.key())]
    pub program: AccountInfo<'info>,
}
