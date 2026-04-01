use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_lang::solana_program::program_pack::Pack;
use anchor_lang::solana_program::system_instruction;
use anchor_spl::associated_token::{self, AssociatedToken};
use anchor_spl::token::{self, MintTo, Token};
use mpl_token_metadata::instructions::{
    CreateMasterEditionV3CpiBuilder, CreateMetadataAccountV3CpiBuilder,
};
use mpl_token_metadata::types::DataV2;

declare_id!("A8CYAG3UCBq8WBLpwkgb61aEDB3fdQn26BFpZr3HQuUr");

const CONFIG_SEED: &[u8] = b"item-config";
const ITEM_AUTHORITY_SEED: &[u8] = b"item-authority";

#[program]
pub mod item_nft {
    use super::*;

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        crafting_authority: Pubkey,
        marketplace_authority: Pubkey,
    ) -> Result<()> {
        let (_, item_authority_bump) =
            Pubkey::find_program_address(&[ITEM_AUTHORITY_SEED], ctx.program_id);

        let config = &mut ctx.accounts.config;
        config.admin = ctx.accounts.admin.key();
        config.crafting_authority = crafting_authority;
        config.marketplace_authority = marketplace_authority;
        config.item_authority_bump = item_authority_bump;
        config.bump = ctx.bumps.config;
        Ok(())
    }

    pub fn mint_item(
        ctx: Context<MintItem>,
        item_type: u8,
        item_nonce: u64,
        name: String,
        symbol: String,
        uri: String,
    ) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.caller_authority.key(),
            ctx.accounts.config.crafting_authority,
            ItemNftError::UnauthorizedCaller
        );

        let item_authority_seeds: &[&[u8]] = &[
            ITEM_AUTHORITY_SEED,
            &[ctx.accounts.config.item_authority_bump],
        ];

        let rent = Rent::get()?;
        let mint_space = anchor_spl::token::spl_token::state::Mint::LEN;
        let mint_lamports = rent.minimum_balance(mint_space);

        if ctx.accounts.item_mint.data_is_empty() {
            invoke(
                &system_instruction::create_account(
                    &ctx.accounts.owner.key(),
                    &ctx.accounts.item_mint.key(),
                    mint_lamports,
                    mint_space as u64,
                    &ctx.accounts.token_program.key(),
                ),
                &[
                    ctx.accounts.owner.to_account_info(),
                    ctx.accounts.item_mint.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
            )?;

            let init_mint_ix = anchor_spl::token::spl_token::instruction::initialize_mint2(
                &ctx.accounts.token_program.key(),
                &ctx.accounts.item_mint.key(),
                &ctx.accounts.item_authority.key(),
                Some(&ctx.accounts.item_authority.key()),
                0,
            )?;

            invoke(&init_mint_ix, &[ctx.accounts.item_mint.to_account_info()])?;
        }

        if ctx.accounts.owner_item_token_account.data_is_empty() {
            associated_token::create(CpiContext::new(
                ctx.accounts.associated_token_program.to_account_info(),
                associated_token::Create {
                    payer: ctx.accounts.owner.to_account_info(),
                    associated_token: ctx.accounts.owner_item_token_account.to_account_info(),
                    authority: ctx.accounts.owner.to_account_info(),
                    mint: ctx.accounts.item_mint.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                },
            ))?;
        }

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.item_mint.to_account_info(),
                    to: ctx.accounts.owner_item_token_account.to_account_info(),
                    authority: ctx.accounts.item_authority.to_account_info(),
                },
                &[item_authority_seeds],
            ),
            1,
        )?;

        let metadata_data = DataV2 {
            name,
            symbol,
            uri,
            seller_fee_basis_points: 0,
            creators: None,
            collection: None,
            uses: None,
        };

        CreateMetadataAccountV3CpiBuilder::new(&ctx.accounts.metadata_program.to_account_info())
            .metadata(&ctx.accounts.metadata_account.to_account_info())
            .mint(&ctx.accounts.item_mint.to_account_info())
            .mint_authority(&ctx.accounts.item_authority.to_account_info())
            .payer(&ctx.accounts.owner.to_account_info())
            .update_authority(&ctx.accounts.item_authority.to_account_info(), true)
            .system_program(&ctx.accounts.system_program.to_account_info())
            .rent(Some(&ctx.accounts.rent.to_account_info()))
            .data(metadata_data)
            .is_mutable(true)
            .invoke_signed(&[item_authority_seeds])?;

        CreateMasterEditionV3CpiBuilder::new(&ctx.accounts.metadata_program.to_account_info())
            .edition(&ctx.accounts.master_edition.to_account_info())
            .mint(&ctx.accounts.item_mint.to_account_info())
            .update_authority(&ctx.accounts.item_authority.to_account_info())
            .mint_authority(&ctx.accounts.item_authority.to_account_info())
            .payer(&ctx.accounts.owner.to_account_info())
            .metadata(&ctx.accounts.metadata_account.to_account_info())
            .token_program(&ctx.accounts.token_program.to_account_info())
            .system_program(&ctx.accounts.system_program.to_account_info())
            .rent(Some(&ctx.accounts.rent.to_account_info()))
            .max_supply(0)
            .invoke_signed(&[item_authority_seeds])?;

        let item_record = &mut ctx.accounts.item_record;
        item_record.owner = ctx.accounts.owner.key();
        item_record.mint = ctx.accounts.item_mint.key();
        item_record.item_type = item_type;
        item_record.item_nonce = item_nonce;
        item_record.burned = false;
        item_record.bump = ctx.bumps.item_record;

        Ok(())
    }

    pub fn burn_item(ctx: Context<BurnItem>) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.caller_authority.key(),
            ctx.accounts.config.marketplace_authority,
            ItemNftError::UnauthorizedCaller
        );
        require_keys_eq!(
            ctx.accounts.item_record.owner,
            ctx.accounts.owner.key(),
            ItemNftError::InvalidOwner
        );
        require!(
            !ctx.accounts.item_record.burned,
            ItemNftError::AlreadyBurned
        );
        require_keys_eq!(
            ctx.accounts.item_record.mint,
            ctx.accounts.item_mint.key(),
            ItemNftError::MintMismatch
        );

        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Burn {
                    mint: ctx.accounts.item_mint.to_account_info(),
                    from: ctx.accounts.owner_item_token_account.to_account_info(),
                    authority: ctx.accounts.owner.to_account_info(),
                },
            ),
            1,
        )?;

        ctx.accounts.item_record.burned = true;
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
        space = ItemNftConfig::LEN,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, ItemNftConfig>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(item_type: u8, item_nonce: u64)]
pub struct MintItem<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, ItemNftConfig>,

    pub caller_authority: Signer<'info>,

    #[account(mut)]
    pub owner: Signer<'info>,

    /// CHECK: PDA authority used only as signer for mint and metadata CPIs.
    #[account(
        seeds = [ITEM_AUTHORITY_SEED],
        bump = config.item_authority_bump
    )]
    pub item_authority: UncheckedAccount<'info>,

    #[account(
        init,
        payer = owner,
        space = ItemRecord::LEN,
        seeds = [b"item-record", owner.key().as_ref(), &item_nonce.to_le_bytes()],
        bump
    )]
    pub item_record: Account<'info, ItemRecord>,

    /// CHECK: mint account is created manually in the instruction.
    #[account(mut)]
    pub item_mint: UncheckedAccount<'info>,

    /// CHECK: ATA is created manually in the instruction when needed.
    #[account(mut)]
    pub owner_item_token_account: UncheckedAccount<'info>,

    /// CHECK: canonical Metaplex metadata PDA for item_mint.
    #[account(mut)]
    pub metadata_account: UncheckedAccount<'info>,

    /// CHECK: canonical Metaplex master edition PDA for item_mint.
    #[account(mut)]
    pub master_edition: UncheckedAccount<'info>,

    /// CHECK: Metaplex token metadata program.
    pub metadata_program: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct BurnItem<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, ItemNftConfig>,

    pub caller_authority: Signer<'info>,

    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut, has_one = owner)]
    pub item_record: Account<'info, ItemRecord>,

    /// CHECK: used only in token burn CPI.
    #[account(mut)]
    pub item_mint: UncheckedAccount<'info>,

    /// CHECK: used only in token burn CPI.
    #[account(mut)]
    pub owner_item_token_account: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[account]
pub struct ItemNftConfig {
    pub admin: Pubkey,
    pub crafting_authority: Pubkey,
    pub marketplace_authority: Pubkey,
    pub item_authority_bump: u8,
    pub bump: u8,
}

impl ItemNftConfig {
    pub const LEN: usize = 8 + 32 + 32 + 32 + 1 + 1;
}

#[account]
pub struct ItemRecord {
    pub owner: Pubkey,
    pub mint: Pubkey,
    pub item_type: u8,
    pub item_nonce: u64,
    pub burned: bool,
    pub bump: u8,
}

impl ItemRecord {
    pub const LEN: usize = 8 + 32 + 32 + 1 + 8 + 1 + 1;
}

#[error_code]
pub enum ItemNftError {
    #[msg("Unauthorized caller")]
    UnauthorizedCaller,
    #[msg("Invalid owner")]
    InvalidOwner,
    #[msg("Item already burned")]
    AlreadyBurned,
    #[msg("Item mint does not match item record")]
    MintMismatch,
}
