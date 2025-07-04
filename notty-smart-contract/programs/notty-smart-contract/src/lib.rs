use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke, system_instruction};
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::metadata::{
    create_metadata_accounts_v3, mpl_token_metadata::types::DataV2, CreateMetadataAccountsV3,
    Metadata,
};
use anchor_spl::token::{self, Mint, MintTo, SetAuthority, Token, TokenAccount, Transfer};
pub const TOKEN_VAULT_SEED: &[u8] = b"token_vault";
pub const SOL_VAULT_SEED: &[u8] = b"sol_vault";
pub const MINT_AUTHORITY_SEED: &[u8] = b"mint_authority";
pub const VAULT_AUTHORITY_SEED: &[u8] = b"authority";
declare_id!("DLL2RN855xoMycXGipog3CjzQqpPBQJ6CpS6hPc8Tj1G");

#[program]
pub mod notty_smart_contract {
    use anchor_lang::solana_program::program::invoke_signed;

    use super::*;

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

        // Transfer mint authority from payer to the PDA -

        // inspiration removed expensive onchain logic
        // let mint_key = ctx.accounts.mint_account.key();
        // let (mint_authority, _) =
        //     Pubkey::find_program_address(&[MINT_AUTHORITY_SEED, mint_key.as_ref()], ctx.program_id);

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
            decimals: 9, // Hardcoded from the mint_account initialization
        });

        Ok(())
    }
    pub fn create_token_with_vault(
        ctx: Context<CreateTokenWithVault>,
        token_name: String,
        token_symbol: String,
        token_uri: String,
        price_per_token: u64,
        initial_supply: u64,
        should_user_buy: bool,
    ) -> Result<()> {
        // 1. Create metadata
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

        // 2. Transfer mint authority to PDA
        let mint_key = ctx.accounts.mint_account.key();
        let (mint_authority, _) =
            Pubkey::find_program_address(&[MINT_AUTHORITY_SEED, mint_key.as_ref()], ctx.program_id);

        let cpi_accounts = SetAuthority {
            account_or_mint: ctx.accounts.mint_account.to_account_info(),
            current_authority: ctx.accounts.payer.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::set_authority(
            cpi_ctx,
            token::spl_token::instruction::AuthorityType::MintTokens,
            Some(mint_authority),
        )?;

        // 3. Mint initial supply to vault
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

        token::mint_to(mint_ctx, initial_supply)?;

        // 4. Store vault metadata
        let vault = &mut ctx.accounts.vault_account;
        vault.mint = ctx.accounts.mint_account.key();
        vault.token_account = ctx.accounts.token_vault.key();
        vault.sol_vault = ctx.accounts.sol_vault.key();
        vault.authority = ctx.accounts.vault_authority.key();
        vault.price_per_token = price_per_token;

        if should_user_buy {
            let forty_percent = initial_supply * 40 / 100;
            let total_price = forty_percent
                .checked_mul(price_per_token)
                .ok_or(ErrorCode::NumericalOverflow)?;

            // Transfer tokens from vault to user's token account (payer's account)
            let token_transfer_accounts = Transfer {
                from: ctx.accounts.token_vault.to_account_info(),
                to: ctx.accounts.payer.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            };

            let seeds: &[&[u8]] = &[
                MINT_AUTHORITY_SEED,
                mint_key.as_ref(),
                &[ctx.bumps.mint_authority],
            ];
            let seed_ref = &[seeds];
            let transfer_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token_transfer_accounts,
                seed_ref,
            );

            token::transfer(transfer_ctx, forty_percent)?;

            // Transfer SOL from payer to vault manually
            **ctx.accounts.payer.try_borrow_mut_lamports()? -= total_price;
            **ctx.accounts.sol_vault.try_borrow_mut_lamports()? += total_price;
        }

        // 5. Emit events
        emit!(TokenWithVaultCreatedEvent {
            token_name,
            token_symbol,
            token_uri,
            mint_address: ctx.accounts.mint_account.key(),
            creator: ctx.accounts.payer.key(),
            decimals: 9,
            initial_supply,
            price_per_token,
        });

        Ok(())
    }
    pub fn init_vault(
        ctx: Context<InitVault>,
        price_per_token: u64,
        initial_supply: u64,
    ) -> Result<()> {
        // Save vault metadata
        let vault = &mut ctx.accounts.vault_account;
        vault.mint = ctx.accounts.mint.key();
        vault.token_account = ctx.accounts.token_vault.key();
        vault.sol_vault = ctx.accounts.sol_vault.key();
        vault.authority = ctx.accounts.vault_authority.key();
        vault.price_per_token = price_per_token;

        // Mint initial tokens into the vault's token account
        let cpi_accounts = MintTo {
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.token_vault.to_account_info(),
            authority: ctx.accounts.mint_authority.to_account_info(),
        };

        let mint_key = ctx.accounts.mint.key();
        let signer_seeds: &[&[&[u8]]] = &[&[
            b"mint_authority",
            mint_key.as_ref(),
            &[ctx.bumps.mint_authority],
        ]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );

        token::mint_to(cpi_ctx, initial_supply)?;

        emit!(InitVaultEvent {
            mint_address: ctx.accounts.mint.key(),
            initial_supply: initial_supply,
            price_per_token: price_per_token
        });
        Ok(())
    }

    pub fn sell_token(ctx: Context<SellToken>, amount: u64) -> Result<()> {
        let decimals = ctx.accounts.mint.decimals;
        let divisor = 10u64.pow(decimals as u32);

        let total_refund = ctx
            .accounts
            .vault_account
            .price_per_token
            .checked_mul(amount)
            .unwrap()
            / divisor;

        // Calculate fee
        let fee_bps = 200;
        let total_fee = total_refund * fee_bps / 10_000;

        let creator_fee = total_fee / 2;
        let owner_fee = total_fee - creator_fee;
        let net_to_seller = total_refund - total_fee;

        // Ensure vault has enough SOL
        require!(
            ctx.accounts.sol_vault.lamports() >= total_refund,
            ErrorCode::VaultInsufficientSol
        );

        // Transfer tokens to vault
        let cpi_accounts = Transfer {
            from: ctx.accounts.seller_token_account.to_account_info(),
            to: ctx.accounts.token_vault.to_account_info(),
            authority: ctx.accounts.seller.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        let vault_seed = &[SOL_VAULT_SEED, &[ctx.bumps.sol_vault]];

        // Pay seller (net)
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
                ctx.accounts.owner.key,
                owner_fee,
            ),
            &[
                ctx.accounts.sol_vault.to_account_info(),
                ctx.accounts.owner.to_account_info(),
            ],
            &[vault_seed],
        )?;
        invoke_signed(
            &system_instruction::transfer(
                ctx.accounts.sol_vault.key,
                ctx.accounts.creator.key,
                creator_fee,
            ),
            &[
                ctx.accounts.sol_vault.to_account_info(),
                ctx.accounts.vault_account.to_account_info(),
            ],
            &[vault_seed],
        )?;

        emit!(TokenTransferEvent {
            transfer_type: 1,
            mint_address: ctx.accounts.mint.key(),
            user: ctx.accounts.seller.key(),
            sol_amount: total_refund,
            coin_amount: amount,
        });

        Ok(())
    }
    pub fn buy_token(ctx: Context<BuyToken>, amount: u64) -> Result<()> {
        let decimals = ctx.accounts.mint.decimals;
        let divisor = 10u64.pow(decimals as u32);

        let total_cost = ctx
            .accounts
            .vault_account
            .price_per_token
            .checked_mul(amount)
            .unwrap()
            / divisor;

        // Calculate fee
        let fee_bps = 200;
        let total_fee = total_cost * fee_bps / 10_000;

        let creator_fee = total_fee / 2;
        let owner_fee = total_fee - creator_fee;
        let net_to_vault = total_cost - total_fee;

        // Check buyer has enough SOL
        require!(
            ctx.accounts.buyer.lamports() >= total_cost,
            ErrorCode::InsufficientFunds
        );

        // Transfer to vault (net)
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
                &ctx.accounts.owner.key(),
                owner_fee,
            ),
            &[
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.owner.to_account_info(),
            ],
        )?;
        invoke(
            &system_instruction::transfer(
                &ctx.accounts.buyer.key(),
                &ctx.accounts.creator.key(),
                creator_fee,
            ),
            &[
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.creator.to_account_info(),
            ],
        )?;

        // SPL token transfer to buyer
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
        token::transfer(cpi_ctx, amount)?;

        emit!(TokenTransferEvent {
            transfer_type: 0,
            mint_address: ctx.accounts.mint.key(),
            user: ctx.accounts.buyer.key(),
            sol_amount: total_cost,
            coin_amount: amount,
        });

        Ok(())
    }

    pub fn migrate_vault(ctx: Context<MigrateVault>) -> Result<()> {
        let vault = &mut ctx.accounts.vault_account;
        require!(!vault.migrated, ErrorCode::AlreadyMigrated);
        let sol_balance = ctx.accounts.sol_vault.to_account_info().lamports();
        require!(
            sol_balance >= vault.price_per_token * vault.token_account_amount(),
            ErrorCode::TargetNotReached
        );

        let cpi_accounts = raydium_launch_cpi::accounts::Initialize {
            pool: ctx.accounts.launchpad_pool.to_account_info(),
            token_vault: ctx.accounts.token_vault.to_account_info(),
            sol_vault: ctx.accounts.sol_vault.to_account_info(),
            authority: ctx.accounts.vault_authority.to_account_info(),
            payer: ctx.accounts.payer.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
            rent: ctx.accounts.rent.to_account_info(),
        };
        let seeds = &[&[
            VAULT_AUTHORITY_SEED,
            vault.mint.as_ref(),
            &[ctx.bumps.vault_authority],
        ]];
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.raydium_program.to_account_info(),
            cpi_accounts,
            seeds,
        );

        raydium_launch_cpi::initialize(cpi_ctx)?;

        vault.migrated = true;
        emit!(VaultMigratedEvent {
            mint: vault.mint,
            pool: ctx.accounts.launchpad_pool.key()
        });
        Ok(())
    }
}

#[event]
pub struct TokenCreatedEvent {
    pub token_name: String,
    pub token_symbol: String,
    pub token_uri: String,
    pub mint_address: Pubkey,
    pub creator: Pubkey, // the user
    pub decimals: u8,
}

#[event]
pub struct TokenTransferEvent {
    pub transfer_type: u8,
    pub mint_address: Pubkey,
    pub user: Pubkey, // the user
    pub sol_amount: u64,
    pub coin_amount: u64,
}
#[event]
pub struct InitVaultEvent {
    pub price_per_token: u64,
    pub initial_supply: u64,
    pub mint_address: Pubkey,
}

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

#[account]
pub struct TokenVault {
    pub mint: Pubkey,
    pub authority: Pubkey,
    pub token_account: Pubkey,
    pub sol_vault: Pubkey,
    pub price_per_token: u64,
    pub migrated: bool,
}

#[derive(Accounts)]
pub struct InitVault<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut)]
    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<TokenVault>(),
        seeds = [b"vault", mint.key().as_ref()],
        bump
    )]
    pub vault_account: Account<'info, TokenVault>,

    #[account(
        init,
        payer = payer,
        seeds = [TOKEN_VAULT_SEED, mint.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = vault_authority
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        seeds = [SOL_VAULT_SEED],
        bump
    )]
    /// CHECK: PDA to receive SOL
    pub sol_vault: UncheckedAccount<'info>,

    #[account(
        seeds = [VAULT_AUTHORITY_SEED, mint.key().as_ref()],
        bump
    )]
    /// CHECK: PDA authority
    pub vault_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [MINT_AUTHORITY_SEED, mint.key().as_ref()],
        bump
    )]
    /// CHECK: PDA for mint authority
    pub mint_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct BuyToken<'info> {
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
        seeds = [b"vault", mint.key().as_ref()],
        bump,
    )]
    pub vault_account: Account<'info, TokenVault>,

    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = mint,
        associated_token::authority = buyer
    )]
    pub buyer_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [SOL_VAULT_SEED],
        bump
    )]
    /// CHECK:
    pub sol_vault: UncheckedAccount<'info>,

    #[account(
        seeds = [VAULT_AUTHORITY_SEED, mint.key().as_ref()],
        bump
    )]
    /// CHECK:
    pub vault_authority: UncheckedAccount<'info>,

    /// CHECK: Global owner of the smart contract
    #[account(mut)]
    pub owner: UncheckedAccount<'info>,

    /// CHECK: Creator of the token (stored in vault_account.creator ideally)
    #[account(mut)]
    pub creator: UncheckedAccount<'info>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SellToken<'info> {
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
        seeds = [b"vault", mint.key().as_ref()],
        bump,
    )]
    pub vault_account: Account<'info, TokenVault>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = seller
    )]
    pub seller_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [SOL_VAULT_SEED],
        bump
    )]
    /// CHECK:
    pub sol_vault: UncheckedAccount<'info>,

    #[account(
        seeds = [VAULT_AUTHORITY_SEED, mint.key().as_ref()],
        bump
    )]
    /// CHECK:
    pub vault_authority: UncheckedAccount<'info>,

    /// CHECK: Global owner of the smart contract
    #[account(mut)]
    pub owner: UncheckedAccount<'info>,

    /// CHECK: Creator of the token (should match vault_account.creator)
    #[account(mut)]
    pub creator: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MigrateVault<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut)]
    pub vault_account: Account<'info, TokenVault>,

    #[account(mut,
        seeds = [TOKEN_VAULT_SEED, vault_account.mint.as_ref()],
        bump)]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(mut,
        seeds = [SOL_VAULT_SEED],
        bump)]
    pub sol_vault: UncheckedAccount<'info>,

    #[account(mut,
        seeds = [VAULT_AUTHORITY_SEED, vault_account.mint.as_ref()],
        bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(mut,
        seeds = [MINT_AUTHORITY_SEED, vault_account.mint.as_ref()],
        bump)]
    pub mint_authority: UncheckedAccount<'info>,

    /// LaunchLab pool PDA to be created
    #[account(mut)]
    pub launchpad_pool: UncheckedAccount<'info>,

    /// Raydium Launch­pad program
    pub raydium_program: Program<'info, raydium_launch_cpi::program::RaydiumLaunch>,

    // Required sysvars/programs
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Not enough SOL to buy tokens")]
    InsufficientFunds,
    #[msg("Vault doesn't have enough SOL to refund")]
    VaultInsufficientSol,
    #[msg("Numerical overflow occurred.")]
    NumericalOverflow,
    #[msg("Liquidity has already been migrated")]
    AlreadyMigrated,
    #[msg("Vault hasn't reached the migration threshold")]
    TargetNotReached,
}

#[derive(Accounts)]
pub struct CreateTokenWithVault<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        payer = payer,
        mint::decimals = 9,
        mint::authority = payer.key(),
        mint::freeze_authority = payer.key(),
    )]
    pub mint_account: Account<'info, Mint>,

    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<TokenVault>(),
        seeds = [b"vault", mint_account.key().as_ref()],
        bump
    )]
    pub vault_account: Account<'info, TokenVault>,

    #[account(
        init,
        payer = payer,
        seeds = [TOKEN_VAULT_SEED, mint_account.key().as_ref()],
        bump,
        token::mint = mint_account,
        token::authority = vault_authority
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        seeds = [SOL_VAULT_SEED],
        bump
    )]
    /// CHECK: PDA to receive SOL
    pub sol_vault: UncheckedAccount<'info>,

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

#[event]
pub struct TokenWithVaultCreatedEvent {
    pub token_name: String,
    pub token_symbol: String,
    pub token_uri: String,
    pub mint_address: Pubkey,
    pub creator: Pubkey,
    pub decimals: u8,
    pub initial_supply: u64,
    pub price_per_token: u64,
}

#[event]
pub struct VaultMigratedEvent {
    pub mint: Pubkey,
    pub pool: Pubkey,          // Raydium pool PDA
    pub sol_deposited: u64,    // SOL amount contributed
    pub tokens_deposited: u64, // SPL tokens deposited
}
