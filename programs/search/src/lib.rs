use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;
use anchor_spl::token_interface::TokenInterface;
use resource_manager::program::ResourceManager;

declare_id!("3nina26x25245eyWTcoRAtxJTeHKvfFHpXwytg7xpJRL");

const PLAYER_SEED: &[u8] = b"player";
const SEARCH_AUTHORITY_SEED: &[u8] = b"search-authority";
const SEARCH_COOLDOWN_SECONDS: i64 = 60;

#[program]
pub mod search {
    use super::*;

    /// Creates the persistent Player PDA used to track on-chain cooldown.
    pub fn initialize_player(ctx: Context<InitializePlayer>) -> Result<()> {
        let player = &mut ctx.accounts.player;
        player.owner = ctx.accounts.owner.key();
        player.last_search_timestamp = 0;
        player.bump = ctx.bumps.player;
        Ok(())
    }

    /// Generates three pseudo-random resource drops and mints them via ResourceManager.
    pub fn search_resources<'info>(
        ctx: Context<'_, '_, '_, 'info, SearchResources<'info>>,
    ) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        let player = &mut ctx.accounts.player;
        require_keys_eq!(
            player.owner,
            ctx.accounts.owner.key(),
            SearchError::InvalidOwner
        );

        if player.last_search_timestamp != 0 {
            let elapsed = current_time - player.last_search_timestamp;
            require!(
                elapsed >= SEARCH_COOLDOWN_SECONDS,
                SearchError::CooldownActive
            );
        }

        let remaining = ctx.remaining_accounts;
        require!(remaining.len() == 12, SearchError::MissingResourceAccounts);

        let (expected_authority, bump) =
            Pubkey::find_program_address(&[SEARCH_AUTHORITY_SEED], ctx.program_id);
        require_keys_eq!(
            expected_authority,
            ctx.accounts.search_authority.key(),
            SearchError::InvalidAuthorityPda
        );

        let drops = derive_resource_ids(
            ctx.accounts.owner.key(),
            current_time,
            player.last_search_timestamp,
            Clock::get()?.slot,
        );

        let authority_seeds: &[&[u8]] = &[SEARCH_AUTHORITY_SEED, &[bump]];
        for resource_id in drops {
            let mint_info = remaining[(resource_id as usize) * 2].clone();
            let ata_info = remaining[(resource_id as usize) * 2 + 1].clone();

            let cpi_accounts = resource_manager::cpi::accounts::MintResourceBySearch {
                config: ctx.accounts.resource_config.to_account_info(),
                caller_authority: ctx.accounts.search_authority.to_account_info(),
                resource_manager_authority: ctx
                    .accounts
                    .resource_manager_authority
                    .to_account_info(),
                resource_mint: mint_info,
                destination_token_account: ata_info,
                token_program: ctx.accounts.token_program.to_account_info(),
            };

            resource_manager::cpi::mint_resource_by_search(
                CpiContext::new_with_signer(
                    ctx.accounts.resource_manager_program.to_account_info(),
                    cpi_accounts,
                    &[authority_seeds],
                ),
                resource_id,
                1,
            )?;
        }

        player.last_search_timestamp = current_time;
        Ok(())
    }
}

fn derive_resource_ids(owner: Pubkey, now: i64, previous: i64, slot: u64) -> [u8; 3] {
    let digest = hashv(&[
        owner.as_ref(),
        &now.to_le_bytes(),
        &previous.to_le_bytes(),
        &slot.to_le_bytes(),
    ]);

    let bytes = digest.to_bytes();
    [bytes[0] % 6, bytes[10] % 6, bytes[20] % 6]
}

#[derive(Accounts)]
pub struct InitializePlayer<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        init,
        payer = owner,
        space = Player::LEN,
        seeds = [PLAYER_SEED, owner.key().as_ref()],
        bump
    )]
    pub player: Account<'info, Player>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SearchResources<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        seeds = [PLAYER_SEED, owner.key().as_ref()],
        bump = player.bump
    )]
    pub player: Account<'info, Player>,
    /// CHECK: Search authority PDA signs CPIs into ResourceManager.
    pub search_authority: UncheckedAccount<'info>,
    /// CHECK: foreign program config.
    pub resource_config: UncheckedAccount<'info>,
    /// CHECK: foreign PDA signer owned by ResourceManager.
    pub resource_manager_authority: UncheckedAccount<'info>,
    pub resource_manager_program: Program<'info, ResourceManager>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[account]
pub struct Player {
    pub owner: Pubkey,
    pub last_search_timestamp: i64,
    pub bump: u8,
}

impl Player {
    pub const LEN: usize = 8 + 32 + 8 + 1;
}

#[error_code]
pub enum SearchError {
    #[msg("Search cooldown is still active")]
    CooldownActive,
    #[msg("Invalid owner")]
    InvalidOwner,
    #[msg("Missing or malformed resource mint/account list")]
    MissingResourceAccounts,
    #[msg("Invalid search authority PDA")]
    InvalidAuthorityPda,
}
