use anchor_lang::prelude::*;
use anchor_lang::prelude::{Interface, InterfaceAccount};
use anchor_lang::solana_program::program::{invoke, invoke_signed};
use anchor_lang::solana_program::system_instruction;
use anchor_spl::token_interface::{self, Mint, MintTo, TokenAccount, TokenInterface};
use spl_pod::optional_keys::OptionalNonZeroPubkey;
use spl_token_2022::extension::metadata_pointer::instruction as metadata_pointer_instruction;
use spl_token_2022::extension::ExtensionType;
use spl_token_2022::state::Mint as SplMint;
use spl_token_metadata_interface::instruction as token_metadata_instruction;
use spl_token_metadata_interface::state::TokenMetadata;
use spl_type_length_value::variable_len_pack::VariableLenPack;

declare_id!("5GXpqYQkcnAfNmX1EQ3A3MGibsK4F1EksUXLGDvVUXXJ");

const CONFIG_SEED: &[u8] = b"magic-config";
const MINT_AUTHORITY_SEED: &[u8] = b"magic-mint-authority";

#[program]
pub mod magic_token {
    use super::*;

    /// Initializes MagicToken controller config.
    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        marketplace_authority: Pubkey,
    ) -> Result<()> {
        let (_, mint_authority_bump) =
            Pubkey::find_program_address(&[MINT_AUTHORITY_SEED], ctx.program_id);
        let config = &mut ctx.accounts.config;
        config.admin = ctx.accounts.admin.key();
        config.marketplace_authority = marketplace_authority;
        config.magic_mint = Pubkey::default();
        config.mint_authority_bump = mint_authority_bump;
        config.bump = ctx.bumps.config;
        Ok(())
    }

    /// Creates the Token-2022 mint used as MagicToken.
    pub fn initialize_mint(
        ctx: Context<InitializeMint>,
        name: String,
        symbol: String,
        uri: String,
    ) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.config.admin,
            ctx.accounts.admin.key(),
            MagicTokenError::Unauthorized
        );
        let metadata = TokenMetadata {
            update_authority: OptionalNonZeroPubkey::try_from(Some(ctx.accounts.admin.key()))
                .map_err(|_| error!(MagicTokenError::InvalidMetadata))?,
            mint: ctx.accounts.magic_mint.key(),
            name: name.clone(),
            symbol: symbol.clone(),
            uri: uri.clone(),
            additional_metadata: vec![],
        };

        let mint_space =
            ExtensionType::try_calculate_account_len::<SplMint>(&[ExtensionType::MetadataPointer])
                .map_err(|_| error!(MagicTokenError::InvalidMetadata))?;
        let metadata_space = metadata
            .tlv_size_of()
            .map_err(|_| error!(MagicTokenError::InvalidMetadata))?;
        let account_space = mint_space + metadata_space;

        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(account_space);

        invoke(
            &system_instruction::create_account(
                &ctx.accounts.admin.key(),
                &ctx.accounts.magic_mint.key(),
                lamports,
                account_space as u64,
                &ctx.accounts.token_program.key(),
            ),
            &[
                ctx.accounts.admin.to_account_info(),
                ctx.accounts.magic_mint.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        let metadata_pointer_ix = metadata_pointer_instruction::initialize(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.magic_mint.key(),
            Some(ctx.accounts.admin.key()),
            Some(ctx.accounts.magic_mint.key()),
        )?;
        invoke(
            &metadata_pointer_ix,
            &[
                ctx.accounts.magic_mint.to_account_info(),
                ctx.accounts.admin.to_account_info(),
            ],
        )?;

        let init_mint_ix = spl_token_2022::instruction::initialize_mint2(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.magic_mint.key(),
            &ctx.accounts.magic_mint_authority.key(),
            Some(&ctx.accounts.magic_mint_authority.key()),
            0,
        )?;
        invoke(&init_mint_ix, &[ctx.accounts.magic_mint.to_account_info()])?;

        let init_metadata_ix = token_metadata_instruction::initialize(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.magic_mint.key(),
            &ctx.accounts.admin.key(),
            &ctx.accounts.magic_mint.key(),
            &ctx.accounts.magic_mint_authority.key(),
            name,
            symbol,
            uri,
        );

        let authority_seeds: &[&[u8]] = &[
            MINT_AUTHORITY_SEED,
            &[ctx.accounts.config.mint_authority_bump],
        ];
        invoke_signed(
            &init_metadata_ix,
            &[
                ctx.accounts.magic_mint.to_account_info(),
                ctx.accounts.admin.to_account_info(),
                ctx.accounts.magic_mint.to_account_info(),
                ctx.accounts.magic_mint_authority.to_account_info(),
            ],
            &[authority_seeds],
        )?;

        ctx.accounts.config.magic_mint = ctx.accounts.magic_mint.key();
        Ok(())
    }

    /// Mints MagicToken rewards only for Marketplace.
    pub fn mint_reward(ctx: Context<MintReward>, amount: u64) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.caller_authority.key(),
            ctx.accounts.config.marketplace_authority,
            MagicTokenError::UnauthorizedCaller
        );
        require_keys_eq!(
            ctx.accounts.magic_mint.key(),
            ctx.accounts.config.magic_mint,
            MagicTokenError::MintMismatch
        );

        let authority_seeds: &[&[u8]] = &[
            MINT_AUTHORITY_SEED,
            &[ctx.accounts.config.mint_authority_bump],
        ];
        token_interface::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.magic_mint.to_account_info(),
                    to: ctx.accounts.recipient_magic_token_account.to_account_info(),
                    authority: ctx.accounts.magic_mint_authority.to_account_info(),
                },
                &[authority_seeds],
            ),
            amount,
        )?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        init,
        payer = admin,
        space = MagicTokenConfig::LEN,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, MagicTokenConfig>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeMint<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, MagicTokenConfig>,
    #[account(mut)]
    pub magic_mint: Signer<'info>,
    /// CHECK: PDA signer for reward minting.
    #[account(
        seeds = [MINT_AUTHORITY_SEED],
        bump = config.mint_authority_bump
    )]
    pub magic_mint_authority: UncheckedAccount<'info>,
    /// CHECK: this must be the Token-2022 program.
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MintReward<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, MagicTokenConfig>,
    pub caller_authority: Signer<'info>,
    /// CHECK: PDA signer for reward minting.
    #[account(
        seeds = [MINT_AUTHORITY_SEED],
        bump = config.mint_authority_bump
    )]
    pub magic_mint_authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub magic_mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub recipient_magic_token_account: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[account]
pub struct MagicTokenConfig {
    pub admin: Pubkey,
    pub marketplace_authority: Pubkey,
    pub magic_mint: Pubkey,
    pub mint_authority_bump: u8,
    pub bump: u8,
}

impl MagicTokenConfig {
    pub const LEN: usize = 8 + 32 + 32 + 32 + 1 + 1;
}

#[error_code]
pub enum MagicTokenError {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Unauthorized caller")]
    UnauthorizedCaller,
    #[msg("Magic mint mismatch")]
    MintMismatch,
}
