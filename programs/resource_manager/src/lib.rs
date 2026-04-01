use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::{invoke, invoke_signed};
use anchor_lang::solana_program::system_instruction;
use anchor_spl::token::{self, Burn, MintTo, Token};
use spl_pod::optional_keys::OptionalNonZeroPubkey;
use spl_token_2022::extension::metadata_pointer::instruction as metadata_pointer_instruction;
use spl_token_2022::extension::ExtensionType;
use spl_token_2022::state::Mint as SplMint;
use spl_token_metadata_interface::instruction as token_metadata_instruction;
use spl_token_metadata_interface::state::TokenMetadata;
use spl_type_length_value::variable_len_pack::VariableLenPack;

declare_id!("83w4MxcqB6XPSvKBFXUfRYTcrDbhe1HAjBRWiyeaz2jB");

const RESOURCE_MANAGER_AUTHORITY_SEED: &[u8] = b"resource-manager-authority";
const CONFIG_SEED: &[u8] = b"resource-config";

#[program]
pub mod resource_manager {
    use super::*;

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        search_authority: Pubkey,
        crafting_authority: Pubkey,
        item_prices: [u64; 4],
    ) -> Result<()> {
        let (_, authority_bump) =
            Pubkey::find_program_address(&[RESOURCE_MANAGER_AUTHORITY_SEED], ctx.program_id);

        let config = &mut ctx.accounts.config;
        config.admin = ctx.accounts.admin.key();
        config.search_authority = search_authority;
        config.crafting_authority = crafting_authority;
        config.resource_manager_authority_bump = authority_bump;
        config.resource_mints = [Pubkey::default(); 6];
        config.item_prices = item_prices;
        config.bump = ctx.bumps.config;
        Ok(())
    }

    pub fn init_resource_mint(
        ctx: Context<InitResourceMint>,
        resource_id: u8,
        name: String,
        symbol: String,
        uri: String,
    ) -> Result<()> {
        require!(resource_id < 6, ResourceManagerError::InvalidResourceId);

        let config = &mut ctx.accounts.config;
        require_keys_eq!(
            config.admin,
            ctx.accounts.admin.key(),
            ResourceManagerError::Unauthorized
        );

        let metadata = TokenMetadata {
            update_authority: OptionalNonZeroPubkey::try_from(Some(ctx.accounts.admin.key()))
                .map_err(|_| error!(ResourceManagerError::InvalidMetadata))?,
            mint: ctx.accounts.resource_mint.key(),
            name: name.clone(),
            symbol: symbol.clone(),
            uri: uri.clone(),
            additional_metadata: vec![],
        };

        let mint_space =
            ExtensionType::try_calculate_account_len::<SplMint>(&[ExtensionType::MetadataPointer])
                .map_err(|_| error!(ResourceManagerError::InvalidMetadata))?;
        let metadata_space = metadata
            .tlv_size_of()
            .map_err(|_| error!(ResourceManagerError::InvalidMetadata))?;
        let account_space = mint_space + metadata_space;

        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(account_space);

        invoke(
            &system_instruction::create_account(
                &ctx.accounts.admin.key(),
                &ctx.accounts.resource_mint.key(),
                lamports,
                account_space as u64,
                &ctx.accounts.token_program.key(),
            ),
            &[
                ctx.accounts.admin.to_account_info(),
                ctx.accounts.resource_mint.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        let metadata_pointer_ix = metadata_pointer_instruction::initialize(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.resource_mint.key(),
            Some(ctx.accounts.admin.key()),
            Some(ctx.accounts.resource_mint.key()),
        )?;
        invoke(
            &metadata_pointer_ix,
            &[
                ctx.accounts.resource_mint.to_account_info(),
                ctx.accounts.admin.to_account_info(),
            ],
        )?;

        let init_mint_ix = spl_token_2022::instruction::initialize_mint2(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.resource_mint.key(),
            &ctx.accounts.resource_manager_authority.key(),
            Some(&ctx.accounts.resource_manager_authority.key()),
            0,
        )?;
        invoke(
            &init_mint_ix,
            &[ctx.accounts.resource_mint.to_account_info()],
        )?;

        let init_metadata_ix = token_metadata_instruction::initialize(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.resource_mint.key(),
            &ctx.accounts.admin.key(),
            &ctx.accounts.resource_mint.key(),
            &ctx.accounts.resource_manager_authority.key(),
            name,
            symbol,
            uri,
        );

        let authority_seeds: &[&[u8]] = &[
            RESOURCE_MANAGER_AUTHORITY_SEED,
            &[config.resource_manager_authority_bump],
        ];

        invoke_signed(
            &init_metadata_ix,
            &[
                ctx.accounts.resource_mint.to_account_info(),
                ctx.accounts.admin.to_account_info(),
                ctx.accounts.resource_mint.to_account_info(),
                ctx.accounts.resource_manager_authority.to_account_info(),
            ],
            &[authority_seeds],
        )?;

        config.resource_mints[resource_id as usize] = ctx.accounts.resource_mint.key();
        Ok(())
    }

    pub fn mint_resource_by_search(
        ctx: Context<MintResourceBySearch>,
        resource_id: u8,
        amount: u64,
    ) -> Result<()> {
        require!(resource_id < 6, ResourceManagerError::InvalidResourceId);
        require_keys_eq!(
            ctx.accounts.caller_authority.key(),
            ctx.accounts.config.search_authority,
            ResourceManagerError::UnauthorizedCaller
        );
        require_keys_eq!(
            ctx.accounts.resource_mint.key(),
            ctx.accounts.config.resource_mints[resource_id as usize],
            ResourceManagerError::ResourceMintMismatch
        );

        let authority_seeds: &[&[u8]] = &[
            RESOURCE_MANAGER_AUTHORITY_SEED,
            &[ctx.accounts.config.resource_manager_authority_bump],
        ];

        let cpi_accounts = MintTo {
            mint: ctx.accounts.resource_mint.to_account_info(),
            to: ctx.accounts.destination_token_account.to_account_info(),
            authority: ctx.accounts.resource_manager_authority.to_account_info(),
        };

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                cpi_accounts,
                &[authority_seeds],
            ),
            amount,
        )?;

        Ok(())
    }

    pub fn burn_resource_by_crafting(
        ctx: Context<BurnResourceByCrafting>,
        amount: u64,
    ) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.caller_authority.key(),
            ctx.accounts.config.crafting_authority,
            ResourceManagerError::UnauthorizedCaller
        );

        let player_token_account_data = ctx
            .accounts
            .player_resource_token_account
            .try_borrow_data()?;
        let parsed_token_account =
            anchor_spl::token::TokenAccount::try_deserialize(&mut &player_token_account_data[..])
                .map_err(|_| error!(ResourceManagerError::InvalidPlayerTokenAccount))?;

        require_keys_eq!(
            parsed_token_account.owner,
            ctx.accounts.player.key(),
            ResourceManagerError::InvalidPlayerTokenAccount
        );

        let cpi_accounts = Burn {
            mint: ctx.accounts.resource_mint.to_account_info(),
            from: ctx.accounts.player_resource_token_account.to_account_info(),
            authority: ctx.accounts.player.to_account_info(),
        };

        token::burn(
            CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts),
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
        space = ResourceConfig::LEN,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, ResourceConfig>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitResourceMint<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, ResourceConfig>,

    #[account(mut)]
    pub resource_mint: Signer<'info>,

    /// CHECK: PDA signer for mint authority and metadata authority.
    #[account(
        seeds = [RESOURCE_MANAGER_AUTHORITY_SEED],
        bump = config.resource_manager_authority_bump
    )]
    pub resource_manager_authority: UncheckedAccount<'info>,

    /// CHECK: token program account used only for CPI / instruction initialization.
    pub token_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MintResourceBySearch<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, ResourceConfig>,

    pub caller_authority: Signer<'info>,

    /// CHECK: PDA signer for mint CPI.
    #[account(
        seeds = [RESOURCE_MANAGER_AUTHORITY_SEED],
        bump = config.resource_manager_authority_bump
    )]
    pub resource_manager_authority: UncheckedAccount<'info>,

    /// CHECK: validated against config.resource_mints and used only in token CPI.
    #[account(mut)]
    pub resource_mint: UncheckedAccount<'info>,

    /// CHECK: destination token account used only in token CPI.
    #[account(mut)]
    pub destination_token_account: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct BurnResourceByCrafting<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, ResourceConfig>,

    pub caller_authority: Signer<'info>,
    pub player: Signer<'info>,

    /// CHECK: resource mint used only in token CPI.
    #[account(mut)]
    pub resource_mint: UncheckedAccount<'info>,

    /// CHECK: token account is manually parsed and validated against player.
    #[account(mut)]
    pub player_resource_token_account: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[account]
pub struct ResourceConfig {
    pub admin: Pubkey,
    pub search_authority: Pubkey,
    pub crafting_authority: Pubkey,
    pub resource_manager_authority_bump: u8,
    pub resource_mints: [Pubkey; 6],
    pub item_prices: [u64; 4],
    pub bump: u8,
}

impl ResourceConfig {
    pub const LEN: usize = 8 + 32 + 32 + 32 + 1 + (32 * 6) + (8 * 4) + 1;
}

#[error_code]
pub enum ResourceManagerError {
    #[msg("Invalid resource id")]
    InvalidResourceId,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Unauthorized caller")]
    UnauthorizedCaller,
    #[msg("Resource mint mismatch")]
    ResourceMintMismatch,
    #[msg("Invalid player token account")]
    InvalidPlayerTokenAccount,
}
