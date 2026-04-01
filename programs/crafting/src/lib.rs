use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::Token;
use anchor_spl::token_interface::TokenInterface;
use item_nft::program::ItemNft;
use resource_manager::program::ResourceManager;

declare_id!("3c8n9uxWmQ3N6S9Vukm3VeQMvo2JXrxMKGbHsqJVt5Jn");

const CRAFTING_AUTHORITY_SEED: &[u8] = b"crafting-authority";

#[program]
pub mod crafting {
    use super::*;

    pub fn craft_item<'info>(
        ctx: Context<'_, '_, '_, 'info, CraftItem<'info>>,
        item_type: u8,
        item_nonce: u64,
        uri: String,
    ) -> Result<()> {
        let (expected_authority, bump) =
            Pubkey::find_program_address(&[CRAFTING_AUTHORITY_SEED], ctx.program_id);

        require_keys_eq!(
            expected_authority,
            ctx.accounts.crafting_authority.key(),
            CraftingError::InvalidAuthorityPda
        );

        let recipe = recipe_for(item_type)?;
        require!(
            ctx.remaining_accounts.len() == 12,
            CraftingError::MissingResourceAccounts
        );

        let signer_seeds: &[&[u8]] = &[CRAFTING_AUTHORITY_SEED, &[bump]];

        let remaining = ctx.remaining_accounts;

        for (resource_id, amount) in recipe.iter().enumerate() {
            if *amount == 0 {
                continue;
            }

            let mint_info = remaining[resource_id * 2].clone();
            let user_ata_info = remaining[resource_id * 2 + 1].clone();

            let cpi_accounts = resource_manager::cpi::accounts::BurnResourceByCrafting {
                config: ctx.accounts.resource_config.to_account_info(),
                caller_authority: ctx.accounts.crafting_authority.to_account_info(),
                player: ctx.accounts.owner.to_account_info(),
                resource_mint: mint_info,
                player_resource_token_account: user_ata_info,
                token_program: ctx.accounts.token_2022_program.to_account_info(),
            };

            resource_manager::cpi::burn_resource_by_crafting(
                CpiContext::new_with_signer(
                    ctx.accounts.resource_manager_program.to_account_info(),
                    cpi_accounts,
                    &[signer_seeds],
                ),
                *amount,
            )?;
        }

        let (name, symbol) = item_identity(item_type)?;

        let mint_item_accounts = item_nft::cpi::accounts::MintItem {
            config: ctx.accounts.item_nft_config.to_account_info(),
            caller_authority: ctx.accounts.crafting_authority.to_account_info(),
            owner: ctx.accounts.owner.to_account_info(),
            item_authority: ctx.accounts.item_authority.to_account_info(),
            item_record: ctx.accounts.item_record.to_account_info(),
            item_mint: ctx.accounts.item_mint.to_account_info(),
            owner_item_token_account: ctx.accounts.owner_item_token_account.to_account_info(),
            metadata_account: ctx.accounts.metadata_account.to_account_info(),
            master_edition: ctx.accounts.master_edition.to_account_info(),
            metadata_program: ctx.accounts.metadata_program.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
            associated_token_program: ctx.accounts.associated_token_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            rent: ctx.accounts.rent.to_account_info(),
        };

        item_nft::cpi::mint_item(
            CpiContext::new_with_signer(
                ctx.accounts.item_nft_program.to_account_info(),
                mint_item_accounts,
                &[signer_seeds],
            ),
            item_type,
            item_nonce,
            name.to_string(),
            symbol.to_string(),
            uri,
        )?;

        Ok(())
    }
}

fn recipe_for(item_type: u8) -> Result<[u64; 6]> {
    match item_type {
        0 => Ok([1, 3, 0, 1, 0, 0]), // WOOD, IRON, GOLD, LEATHER, STONE, DIAMOND
        1 => Ok([2, 0, 1, 0, 0, 1]),
        2 => Ok([0, 2, 1, 4, 0, 0]),
        3 => Ok([0, 4, 2, 0, 0, 2]),
        _ => err!(CraftingError::InvalidItemType),
    }
}

fn item_identity(item_type: u8) -> Result<(&'static str, &'static str)> {
    match item_type {
        0 => Ok(("Shablya kozaka", "SABER")),
        1 => Ok(("Posokh starishyny", "STAFF")),
        2 => Ok(("Bronya kharakternyka", "ARMOR")),
        3 => Ok(("Boyovyi braslet", "BRACELET")),
        _ => err!(CraftingError::InvalidItemType),
    }
}

#[derive(Accounts)]
pub struct CraftItem<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    /// CHECK: Crafting authority PDA signs CPIs to ResourceManager and ItemNFT.
    pub crafting_authority: UncheckedAccount<'info>,

    /// CHECK: foreign ResourceManager config account.
    pub resource_config: UncheckedAccount<'info>,

    /// CHECK: foreign ResourceManager signer PDA.
    pub resource_manager_authority: UncheckedAccount<'info>,

    /// CHECK: foreign ItemNFT config.
    pub item_nft_config: UncheckedAccount<'info>,

    /// CHECK: foreign ItemNFT signer PDA.
    pub item_authority: UncheckedAccount<'info>,

    /// CHECK: created inside ItemNFT CPI.
    #[account(mut)]
    pub item_record: UncheckedAccount<'info>,

    /// CHECK: created inside ItemNFT CPI.
    #[account(mut)]
    pub item_mint: UncheckedAccount<'info>,

    /// CHECK: ATA created inside ItemNFT CPI.
    #[account(mut)]
    pub owner_item_token_account: UncheckedAccount<'info>,

    /// CHECK: Metaplex metadata PDA for the item mint.
    #[account(mut)]
    pub metadata_account: UncheckedAccount<'info>,

    /// CHECK: Metaplex master edition PDA for the item mint.
    #[account(mut)]
    pub master_edition: UncheckedAccount<'info>,

    /// CHECK: Metaplex token metadata program.
    pub metadata_program: UncheckedAccount<'info>,

    pub resource_manager_program: Program<'info, ResourceManager>,
    pub item_nft_program: Program<'info, ItemNft>,
    pub token_2022_program: Interface<'info, TokenInterface>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[error_code]
pub enum CraftingError {
    #[msg("Invalid crafting authority PDA")]
    InvalidAuthorityPda,
    #[msg("Invalid item type")]
    InvalidItemType,
    #[msg("Missing resource mint/account list")]
    MissingResourceAccounts,
}
